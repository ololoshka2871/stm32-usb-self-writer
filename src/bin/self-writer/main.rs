#![no_std]
#![no_main]

mod impls;
mod init;
mod types;

extern crate alloc;

use alloc::vec::Vec;

use defmt_rtt as _; // global logger
use panic_abort as _;
use self_recorder_packet::{DataBlockPacker, PushResult};

use stm32l4xx_hal::{
    dma::dma1,
    gpio::{Alternate, Output, PA5, PA8, PD10, PD13, PushPull},
    pac::{TIM1, TIM2},
    prelude::*,
};

use rtic::app;
use rtic_monotonics::Monotonic;
use rtic_sync::channel::{Receiver, Sender};

use stm32_usb_self_writer::{
    InputChannel, RtcSync,
    clocking::{I2CRtcCtrl, rtc::RtcService},
    config, is_usb_connected,
    sensors::freqmeter::{Capture, Capturer, ExtInputType, TimerInpitCounterExt},
    settings,
    support::{
        crc::{STM32L4Crc32, STM32L4Crc32Handler, ZlibCompantCrc32},
        usb_periph::UsbPeriph,
    },
    workmodes::FChannel,
};

use init::*;

//-----------------------------------------------------------------------------

rtic_monotonics::systick_monotonic!(Mono, config::SYST_TIMER_HZ);

//-----------------------------------------------------------------------------

defmt::timestamp!(
    "[T{=u32:ms}]",
    Mono::now().ticks() * (1_000 / config::SYST_TIMER_HZ)
);

//-----------------------------------------------------------------------------

static mut HEAP: [u8; config::HEAP_SIZE] = [0; config::HEAP_SIZE];

const RES_QUEUE_SIZE: usize = 192;

//-----------------------------------------------------------------------------

#[app(device = stm32l4xx_hal::pac, peripherals = true, dispatchers = [RCC, LCD, TAMP_STAMP, SWPMI1])]
mod app {
    use super::*;

    #[shared]
    struct Shared {
        led: types::Led,
        rtc: RtcService,
        base_period: config::Duration,
        f1_base_period_devider: u32,
        f2_base_period_devider: u32,
        rtc_sync: RtcSync<Mono>,

        analog_sens: stm32_usb_self_writer::sensors::analog::AnalogSensor<types::VBatPin>,

        master_counter_freq: stm32l4xx_hal::time::Hertz,

        transfer_fin1: dma1::C2,
        f1_capturer: Capturer<TIM2, PA5<Alternate<PushPull, 1>>, { ExtInputType::TI1FP1 as u8 }>,

        transfer_fin2: dma1::C6,
        f2_capturer: Capturer<TIM1, PA8<Alternate<PushPull, 1>>, { ExtInputType::TI1FP1 as u8 }>,

        settings: settings::SettingsManagerType,
        flash_policy: settings::FlasRWPolcy<settings::AppSettings, STM32L4Crc32Handler>,

        usb_dev: usb_device::device::UsbDevice<'static, stm32_usbd::UsbBus<UsbPeriph>>,
        scsi: usbd_scsi::Scsi<
            'static,
            stm32_usbd::UsbBus<UsbPeriph>,
            stm32_usb_self_writer::vfs::EMfatStorage,
        >,
        serial: usbd_serial::CdcAcmClass<'static, stm32_usbd::UsbBus<UsbPeriph>>,
        usb_notify: no_std_async::Condvar,

        output_storage: stm32_usb_self_writer::workmodes::output_storage::OutputStorage,
    }

    #[local]
    struct Local {
        crc_handler: STM32L4Crc32Handler,

        storage_meta: stm32_usb_self_writer::main_data_storage::StorageMetaHandle,
        storage_meta_usb: stm32_usb_self_writer::main_data_storage::StorageMetaHandle,
        storage_erase: stm32_usb_self_writer::main_data_storage::StorageEraseHandle,

        master_timer: types::MasterCounter,
        f1_power_pin: PD10<Output<PushPull>>,
        f2_power_pin: PD13<Output<PushPull>>,

        f1_capture_buffer: &'static mut types::MasterCounterType,
        f1_capture_tx: Sender<'static, Capture, 1>,
        f1_capture_rx: Receiver<'static, Capture, 1>,

        f2_capture_buffer: &'static mut types::MasterCounterType,
        f2_capture_tx: Sender<'static, Capture, 1>,
        f2_capture_rx: Receiver<'static, Capture, 1>,

        protobuf_input_rx: Receiver<'static, Vec<u8>, 4>,
        protobuf_input_tx: Sender<'static, Vec<u8>, 4>,
        protobuf_output_tx: Sender<'static, Vec<u8>, 1>,
        protobuf_output_rx: Receiver<'static, Vec<u8>, 1>,

        self_writer_data_tx: Sender<'static, types::DatItem, RES_QUEUE_SIZE>,
        self_writer_data_rx: Receiver<'static, types::DatItem, RES_QUEUE_SIZE>,

        prepare_delay: config::Duration,

        storage_context: Option<stm32_usb_self_writer::main_data_storage::StorageContext>,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local) {
        let mut dp = ctx.device;

        #[cfg(feature = "force-defmt-logs")]
        // need for defmt logging works https://github.com/knurling-rs/probe-run/pull/183/files
        dp.RCC.ahb1enr.modify(|_, w| w.dma1en().set_bit());

        defmt::info!("+ Init +");

        ctx.core.DCB.enable_trace();
        ctx.core.DWT.enable_cycle_counter();
        defmt::info!("\tDWT");

        let fast_mode = is_usb_connected();

        let mut flash = dp.FLASH.constrain();
        let mut rcc = dp.RCC.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);

        let (clocks, master_counter_freq, high_perf_mode) =
            init_clocks(fast_mode, &mut flash, &mut rcc, &mut pwr);

        unsafe {
            #[allow(static_mut_refs)]
            umm_malloc::init_heap(HEAP.as_mut_ptr() as usize, config::HEAP_SIZE)
        };

        defmt::info!("\tHeap");

        // Initialize the systick interrupt & obtain the token to prove that we did
        Mono::start(ctx.core.SYST, clocks.hclk().to_Hz());
        defmt::info!("\tSysTick");

        let crc = {
            let crc = STM32L4Crc32::new(dp.CRC.constrain(&mut rcc.ahb1));
            cortex_m::singleton!(: STM32L4Crc32 = crc).unwrap()
        };

        let (settings, flash_policy, base_period, start_delay, write_config) =
            init_settings(flash, unsafe { crc.make_handler() }, high_perf_mode);

        let mut rtc = init_rtc_service(
            base_period,
            dp.RTC,
            &mut dp.EXTI,
            &mut rcc.apb1r1,
            &mut rcc.bdcr,
            &mut pwr.cr1,
        );

        #[allow(dead_code, unused_mut)]
        let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);
        #[allow(dead_code, unused_mut)]
        let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);
        #[allow(dead_code, unused_mut)]
        let mut gpioc = dp.GPIOC.split(&mut rcc.ahb2);
        #[allow(dead_code, unused_mut)]
        let mut gpiod = dp.GPIOD.split(&mut rcc.ahb2);
        #[allow(dead_code, unused_mut)]
        let mut _gpioe = dp.GPIOE.split(&mut rcc.ahb2);

        let analog_sens = init_analog_sensors(
            &clocks,
            dp.ADC1,
            dp.ADC_COMMON,
            &mut rcc.ahb2,
            &mut rcc.ccipr,
            gpioa.pa1.into_analog(&mut gpioa.moder, &mut gpioa.pupdr),
        );

        let master_timer = init_master_timer(dp.TIM6, clocks, &mut rcc.apb1r1);

        let dma1 = dp.DMA1.split(&mut rcc.ahb1);

        let (transfer_fin1, f1_capturer, f1_capture_buffer, (f1_capture_tx, f1_capture_rx)) = stm32_usb_self_writer::build_freqmeter_dma!(
            input_timer = dp
                .TIM2
                .into_input_counter(gpioa.pa5.into_alternate_push_pull(
                    &mut gpioa.moder,
                    &mut gpioa.otyper,
                    &mut gpioa.afrl
                )),
            dma_channel = dma1.2, // DMA1 Channel 2[CxS=4] is connected to TIM2_UP
            master_timer = master_timer,
            master_type = types::MasterCounterType,
            dp = dp,
            stop_reg = apb1fzr1,
            stop_bit = dbg_tim2_stop
        );
        let f1_power_pin = gpiod.pd10.into_push_pull_output_in_state(
            &mut gpiod.moder,
            &mut gpiod.otyper,
            config::GENERATOR_DISABLE_LVL,
        );
        defmt::info!("\tFreqmeter 1");

        let (transfer_fin2, f2_capturer, f2_capture_buffer, (f2_capture_tx, f2_capture_rx)) = stm32_usb_self_writer::build_freqmeter_dma!(
            input_timer = dp
                .TIM1
                .into_input_counter(gpioa.pa8.into_alternate_push_pull(
                    &mut gpioa.moder,
                    &mut gpioa.otyper,
                    &mut gpioa.afrh
                )),
            dma_channel = dma1.6, // DMA1 Channel 6[CxS=7] is connected to TIM1_UP
            master_timer = master_timer,
            master_type = types::MasterCounterType,
            dp = dp,
            stop_reg = apb2fzr,
            stop_bit = dbg_tim1_stop
        );
        let f2_power_pin = gpiod.pd13.into_push_pull_output_in_state(
            &mut gpiod.moder,
            &mut gpiod.otyper,
            config::GENERATOR_DISABLE_LVL,
        );
        defmt::info!("\tFreqmeter 2");

        let mut storage_context = {
            let flash_reset_pin = gpiod
                .pd11
                .into_push_pull_output(&mut gpiod.moder, &mut gpiod.otyper);
            let clk_pin =
                gpioa
                    .pa3
                    .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);

            let pins_ch1 = (
                gpioa
                    .pa2
                    .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl),
                #[cfg(not(feature = "maket"))]
                gpiob
                    .pb1
                    .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl),
                #[cfg(feature = "maket")]
                _gpioe
                    .pe12
                    .into_alternate(&mut _gpioe.moder, &mut _gpioe.otyper, &mut _gpioe.afrh),
                gpiob
                    .pb0
                    .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl),
                gpioa
                    .pa7
                    .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl),
                gpioa
                    .pa6
                    .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl),
            );
            let pins_ch2 = (
                gpiod
                    .pd3
                    .into_alternate(&mut gpiod.moder, &mut gpiod.otyper, &mut gpiod.afrl),
                gpiod
                    .pd4
                    .into_alternate(&mut gpiod.moder, &mut gpiod.otyper, &mut gpiod.afrl),
                gpiod
                    .pd5
                    .into_alternate(&mut gpiod.moder, &mut gpiod.otyper, &mut gpiod.afrl),
                gpiod
                    .pd6
                    .into_alternate(&mut gpiod.moder, &mut gpiod.otyper, &mut gpiod.afrl),
                gpiod
                    .pd7
                    .into_alternate(&mut gpiod.moder, &mut gpiod.otyper, &mut gpiod.afrl),
            );

            init_storage::<Mono, _, _, _, _, _, _, _, _, _, _, _, _>(
                unsafe { qspi_stm32lx3::stm32l4x3::QUADSPI::new() },
                flash_reset_pin,
                clk_pin,
                pins_ch1,
                pins_ch2,
                &mut rcc,
                &clocks,
            )
        };

        {
            let mut sda = gpioc.pc0.into_alternate_open_drain(
                &mut gpioc.moder,
                &mut gpioc.otyper,
                &mut gpioc.afrl,
            );
            sda.internal_pull_up(&mut gpioc.pupdr, true);

            let mut scl = gpioc.pc1.into_alternate_open_drain(
                &mut gpioc.moder,
                &mut gpioc.otyper,
                &mut gpioc.afrl,
            );
            scl.internal_pull_up(&mut gpioc.pupdr, true);

            let rtc_i2c: stm32l4xx_hal::i2c::I2c<stm32l4xx_hal::pac::I2C3, (_, _)> =
                stm32l4xx_hal::i2c::I2c::i2c3(
                    dp.I2C3,
                    (sda, scl),
                    stm32l4xx_hal::i2c::Config::new(100_u32.kHz(), clocks),
                    &mut rcc.apb1r1,
                );

            match crate::init::try_init_external_rtc(rtc_i2c) {
                Ok(mut ext_rtc) => {
                    let rtc_time = ext_rtc.current_time().unwrap();
                    defmt::info!("\tExternal RTC detected: {} [{}]", &ext_rtc, rtc_time);

                    rtc.set_time(rtc_time);
                    defmt::warn!("\tInternal RTC time set to match external RTC");
                }
                Err(rtc_i2c) => {
                    defmt::info!("\tNo external RTC detected");
                    let (_, (sda, scl)) = rtc_i2c.free();
                    let _ = sda.into_floating_input(&mut gpioc.moder, &mut gpioc.pupdr);
                    let _ = scl.into_floating_input(&mut gpioc.moder, &mut gpioc.pupdr);
                }
            }
        }

        let storage_meta = storage_context.meta_handle();
        let storage_meta_usb = storage_meta;
        let storage_erase = storage_context.erase_handle();

        let (usb_dev, scsi, serial, mut storage_context) = init_usb(
            fast_mode,
            UsbPeriph {
                usb: dp.USB,
                pin_dm: gpioa
                    .pa11
                    .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh)
                    .set_speed(stm32l4xx_hal::gpio::Speed::VeryHigh),
                pin_dp: gpioa
                    .pa12
                    .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh)
                    .set_speed(stm32l4xx_hal::gpio::Speed::VeryHigh),
            },
            unsafe {
                cortex_m::singleton!(
                    : Option<usb_device::bus::UsbBusAllocator<
                        stm32_usbd::UsbBus<stm32_usb_self_writer::support::usb_periph::UsbPeriph>>
                    > = None
                )
                .unwrap_unchecked()
            },
            usb_device::device::UsbVidPid(0x0483, 0x5720),
            storage_context,
        );

        if !high_perf_mode {
            if let Some(storage_context) = storage_context.as_mut() {
                storage_context
                    .set_mode(stm32_usb_self_writer::main_data_storage::StorageMode::Recorder);
            }
        }

        // test flash memory-maped read hack
        //unsafe {
        //    let mut dptr = [0u8; 128];
        //    let mut ptr = dptr.as_mut_ptr();
        //    stm32_usb_self_writer::qspi_storage::runtime_try_memory_mapped_hack(0, ptr, 512);
        //    let hack =
        //        usbd_scsi::direct_read::DirectReadHack::deserialise_ptr(dptr.as_ptr(), dptr.len())
        //            .expect("Failed to deserialise DirectReadHack from memory-mapped QSPI");
        //    let hacked_ptr: *const u8 = hack.pointer();
        //    defmt::info!(
        //        "first 128 bytes of bank 1: {:x}",
        //        core::slice::from_raw_parts(hacked_ptr, 128)
        //    );
        //
        //    stm32_usb_self_writer::qspi_storage::runtime_try_memory_mapped_hack(4096, ptr, 512);
        //    let hack =
        //        usbd_scsi::direct_read::DirectReadHack::deserialise_ptr(dptr.as_ptr(), dptr.len())
        //            .expect("Failed to deserialise DirectReadHack from memory-mapped QSPI");
        //    let hacked_ptr: *const u8 = hack.pointer();
        //
        //    defmt::info!(
        //        "first 128 bytes of bank 2: {:x}",
        //        core::slice::from_raw_parts(hacked_ptr, 128)
        //    );
        //}

        let (protobuf_input_tx, protobuf_input_rx) = rtic_sync::make_channel!(Vec<u8>, 4);
        let (protobuf_output_tx, protobuf_output_rx) = rtic_sync::make_channel!(Vec<u8>, 1);

        let (self_writer_data_tx, self_writer_data_rx) =
            rtic_sync::make_channel!(types::DatItem, RES_QUEUE_SIZE);

        let led = gpioc.pc10.into_push_pull_output_in_state(
            &mut gpioc.moder,
            &mut gpioc.otyper,
            config::LED_DISABLE,
        );
        defmt::info!("\tLED");

        //---------------------------------------------------------------------

        let prepare_delay = if high_perf_mode {
            sync_freqmeter1::spawn().expect("Failed to spawn sync_freqmeter1 task");
            sync_freqmeter2::spawn().expect("Failed to spawn sync_freqmeter2 task");
            calc_results::spawn().expect("Failed to spawn calc_results task");

            usb_task::spawn().expect("Failed to spawn usb_task task");
            protobuf_server::spawn().expect("Failed to spawn protobuf_server task");
            read_analog::spawn().expect("Failed to spawn read_analog task");

            config::Duration::secs(0)
        } else {
            self_writer_signal::spawn().expect("Failed to spawn self_writer_signal task");

            let blink_delay =
                config::Duration::millis(config::START_BLINK_PERIOD_MS) * config::START_BLINK_COUNT;

            if start_delay > blink_delay {
                start_delay - blink_delay
            } else {
                config::Duration::secs(0)
            }
        };

        defmt::info!("Tasks spawned");

        //---------------------------------------------------------------------

        (
            Shared {
                led,

                rtc,
                base_period,
                f1_base_period_devider: write_config.p_write_devider,
                f2_base_period_devider: write_config.t_write_devider,
                rtc_sync: RtcSync::<Mono>::new(base_period),

                analog_sens,

                master_counter_freq,

                transfer_fin1,
                f1_capturer,

                transfer_fin2,
                f2_capturer,

                settings,
                flash_policy,

                usb_dev,
                scsi,
                serial,
                usb_notify: no_std_async::Condvar::new(),

                output_storage: Default::default(),
            },
            Local {
                crc_handler: unsafe { crc.make_handler() },

                storage_meta,
                storage_meta_usb,
                storage_erase,

                master_timer,

                f1_power_pin,
                f2_power_pin,

                f1_capture_buffer,
                f1_capture_tx,
                f1_capture_rx,
                f2_capture_buffer,
                f2_capture_tx,
                f2_capture_rx,

                protobuf_input_rx,
                protobuf_input_tx,
                protobuf_output_tx,
                protobuf_output_rx,

                self_writer_data_tx,
                self_writer_data_rx,

                prepare_delay,

                storage_context,
            },
        )
    }

    //-------------------------------------------------------------------------

    #[task(binds = TIM6_DAC, local = [master_timer], priority = 7)]
    fn master_timer_ovf(ctx: master_timer_ovf::Context) {
        unsafe { ctx.local.master_timer.overflow_isr() };
    }

    #[task(
        binds=DMA1_CH2,
        shared = [transfer_fin1, f1_capturer],
        local = [f1_capture_buffer, f1_capture_tx],
        priority = 5)
    ]
    fn f1_dma_transfer_complete(mut ctx: f1_dma_transfer_complete::Context) {
        stm32_usb_self_writer::freqmeter_dma_interrupt!(
            channel = InputChannel::Ch1,
            buffer = **ctx.local.f1_capture_buffer,
            capture_tx = ctx.local.f1_capture_tx,
            capturer = ctx.shared.f1_capturer,
            transfer = ctx.shared.transfer_fin1,
        );
    }

    #[task(
        binds=DMA1_CH6,
        shared = [transfer_fin2, f2_capturer],
        local = [f2_capture_buffer, f2_capture_tx],
        priority = 5)
    ]
    fn f2_dma_transfer_complete(mut ctx: f2_dma_transfer_complete::Context) {
        stm32_usb_self_writer::freqmeter_dma_interrupt!(
            channel = InputChannel::Ch2,
            buffer = **ctx.local.f2_capture_buffer,
            capture_tx = ctx.local.f2_capture_tx,
            capturer = ctx.shared.f2_capturer,
            transfer = ctx.shared.transfer_fin2,
        );
    }

    #[task(binds = USB_FS, shared = [&usb_notify], priority = 2)]
    fn usb_fs(ctx: usb_fs::Context) {
        ctx.shared.usb_notify.notify_one();
    }

    // Приоритет строго равен sync_freqmeter*, иначе Deadlock на мьютексе rtc_sync
    #[task(binds = RTC_WKUP, shared = [rtc, &rtc_sync], priority = 4)]
    fn rtc_alarm(ctx: rtc_alarm::Context) {
        let mut rtc = ctx.shared.rtc;
        let rtc_sync = ctx.shared.rtc_sync;

        rtc.lock(|rtc| rtc.handle_alarm_interrupt());

        // Опасность!
        // Если поток, ожидающий rtc_event не сделает любой .await до следующего
        // rtc_event.wait().await, то он сожрет все нотификации в 1 лицо
        rtc_sync.notify_all();
    }

    //-------------------------------------------------------------------------

    #[task(
        shared = [
            transfer_fin1,
            f1_capturer,
            &master_counter_freq,
            &rtc_sync,
            &base_period, &f1_base_period_devider,
            rtc,
            output_storage
        ],
        local = [f1_capture_rx, f1_power_pin],
        priority = 4,
    )]
    async fn sync_freqmeter1(mut ctx: sync_freqmeter1::Context) {
        stm32_usb_self_writer::freqmeter!(
            channel = InputChannel::Ch1,
            output_storage = ctx.shared.output_storage,
            rtc = ctx.shared.rtc,
            rtc_sync = ctx.shared.rtc_sync,
            base_period = *ctx.shared.base_period,
            base_period_devider = *ctx.shared.f1_base_period_devider,
            capture_rx = ctx.local.f1_capture_rx,
            power_pin = ctx.local.f1_power_pin,
            transfer_fin = ctx.shared.transfer_fin1,
            f_capturer = ctx.shared.f1_capturer,
            f_ref = *ctx.shared.master_counter_freq,
            mono = Mono,
        );
    }

    #[task(
        shared = [
            transfer_fin2,
            f2_capturer,
            &master_counter_freq,
            &rtc_sync,
            &base_period, &f2_base_period_devider,
            rtc,
            output_storage
        ],
        local = [f2_capture_rx, f2_power_pin],
        priority = 4,
    )]
    async fn sync_freqmeter2(mut ctx: sync_freqmeter2::Context) {
        stm32_usb_self_writer::freqmeter!(
            channel = InputChannel::Ch2,
            output_storage = ctx.shared.output_storage,
            rtc = ctx.shared.rtc,
            rtc_sync = ctx.shared.rtc_sync,
            base_period = *ctx.shared.base_period,
            base_period_devider = *ctx.shared.f2_base_period_devider,
            capture_rx = ctx.local.f2_capture_rx,
            power_pin = ctx.local.f2_power_pin,
            transfer_fin = ctx.shared.transfer_fin2,
            f_capturer = ctx.shared.f2_capturer,
            f_ref = *ctx.shared.master_counter_freq,
            mono = Mono,
        );
    }

    #[task(shared = [&base_period, output_storage, settings], priority = 3)]
    async fn calc_results(ctx: calc_results::Context) {
        use stm32_usb_self_writer::{
            support::condition_monitor::{ConditionMonitor, Ordering},
            workmodes::FChannel,
        };

        let period = *ctx.shared.base_period;

        let mut output_storage = ctx.shared.output_storage;
        let mut settings = ctx.shared.settings;

        let monitor = ConditionMonitor::<{ Ordering::Greater }>::new(config::OVER_LIMIT_COUNT);
        let mut overpress_monitor = monitor.clone();
        let mut overheat_monitor = monitor.clone();
        let mut cpu_overheat_monitor = monitor.clone();
        let mut over_power_monitor = monitor.clone();

        let mut last_imput_updated = 0u64;

        loop {
            Mono::delay(period).await;

            let mut output = output_storage.lock(|output_storage| output_storage.clone());
            {
                // не пересчитывать результаты, если входные данные не обновились
                let last_updated = output.freq_timestamp();
                if last_updated <= last_imput_updated {
                    continue;
                } else {
                    last_imput_updated = last_updated;
                }
            }

            let s = settings.lock(|settings| settings.ref_mut().0.clone());

            let monitoring = {
                let t = s
                    .t_coefficients
                    .calc(output.frequencys[FChannel::Temperature as usize])
                    + s.t_zero_correction as f64;

                output.values[FChannel::Temperature as usize] = t;

                let p = s.p_coefficients.calc(
                    output.frequencys[FChannel::Pressure as usize],
                    output.frequencys[FChannel::Temperature as usize],
                );
                let p = s.pressure_meassure_units.wrap(p) + s.p_zero_correction as f64;

                output.values[FChannel::Pressure as usize] = p;

                settings::Monitoring {
                    overpress: overpress_monitor.check(p as f32, s.p_work_range.absolute_maximum),
                    overheat: overheat_monitor.check(t as f32, s.t_work_range.absolute_maximum),
                    cpu_overheat: cpu_overheat_monitor
                        .check(output.t_cpu, s.t_cpu_work_range.absolute_maximum),
                    over_power: over_power_monitor
                        .check(output.vbat, s.vbat_work_range.absolute_maximum),
                }
            };

            output_storage.lock(move |output_storage| {
                *output_storage = output;
            });

            if s.monitoring.has_new_flags(&monitoring) {
                // обновились флаги выхода за пределы рабочего диапазона

                defmt::warn!("Monitoring flags updated: {}", monitoring);
                settings.lock(|settings| {
                    let s = settings.ref_mut();
                    s.0.monitoring = monitoring;
                });
                if let Err(_) = settings_saver::spawn() {
                    defmt::error!("Failed to spawn settings_saver task");
                }
            }
        }
    }

    #[task(
        shared = [settings, flash_policy],
        priority = 2,
    )]
    async fn settings_saver(ctx: settings_saver::Context) {
        use flash_settings_rs::StoragePolicy;

        let mut settings = ctx.shared.settings;
        let mut flash_policy = ctx.shared.flash_policy;

        let copy = settings.lock(|settins| settins.ref_mut().0.clone());

        // Это может делаться долго, поэтому отдельный поток с минимальной приоритетностью
        if let Err(e) = flash_policy.lock(move |policy| policy.store(&copy)) {
            defmt::error!("Failed to save settings: {}", defmt::Debug2Format(&e));
        } else {
            defmt::info!("Settings saved");
        }
    }

    #[task(
        shared = [
            &usb_notify,
            usb_dev,
            scsi,
            serial,
            led,
            settings,
        ],
        local = [
            protobuf_input_tx,
            protobuf_output_rx,
            storage_meta_usb,
        ],
        priority = 2
    )]
    async fn usb_task(ctx: usb_task::Context) {
        use alloc::boxed::Box;

        let usb_notify = ctx.shared.usb_notify;

        let mut usb_dev = ctx.shared.usb_dev;
        let mut scsi = ctx.shared.scsi;
        let mut serial = ctx.shared.serial;
        let mut led = ctx.shared.led;
        let mut settings = ctx.shared.settings;

        let protobuf_input_tx = ctx.local.protobuf_input_tx;
        let protobuf_output_rx = ctx.local.protobuf_output_rx;
        let storage_meta_usb = *ctx.local.storage_meta_usb;

        defmt::info!("USB task started");

        let long_wait = config::Duration::millis(10);
        let short_wait = config::Duration::millis(1);

        let mut tx_data = Option::<Vec<u8>>::None;
        let mut wait = long_wait;

        // Проблема: settings имеет время жизни 'a, и его нельзя упаковать в замыкание и в Box
        // Гарантируется, что unsafe_settings_ptr будет использован только в стеке этой функции
        // и не будет передан в другие потоки, поэтому это безопасно
        let unsafe_settings_ptr =
            settings.lock(|settings| settings.ref_mut().0 as *const settings::AppSettings);
        let settings_accessor =
            move || -> settings::AppSettings { unsafe { &*unsafe_settings_ptr }.clone() };

        scsi.lock(move |scsi| {
            scsi.block_device_mut()
                .set_settings_accessor(Box::new(settings_accessor));
        });

        loop {
            led.lock(|led| led.set_state(config::LED_DISABLE));
            Mono::timeout_after(wait, usb_notify.wait()).await.ok();
            led.lock(|led| led.set_state(config::LED_ENABLE));

            wait = long_wait; // default response

            if storage_meta_usb.is_erase_requested() && !storage_meta_usb.erase_in_progress() {
                if let Err(_) = flash_erase::spawn() {
                    // already queued/running
                }
            }

            // Важно! Список передаваемый сюда в том же порядке,
            // что были инициализированы интерфейсы
            let res = (&mut usb_dev, &mut scsi, &mut serial)
                .lock(|usb_dev, scsi, serial| usb_dev.poll(&mut [scsi, serial]));

            if res && !protobuf_input_tx.is_full() {
                while let Ok(data) = serial.lock(|serial| {
                    let mut buf = [0u8; config::BULK_MAX_PACKET_SIZE];
                    serial.read_packet(&mut buf).map(|len| buf[..len].to_vec())
                }) {
                    protobuf_input_tx.send(data).await.ok();
                }

                wait = short_wait; // fast response
            }

            // send data from protobuf server if exists
            if let Some(mut data) = tx_data.take() {
                let to_send = data.len().min(config::BULK_MAX_PACKET_SIZE);
                match serial.lock(|serial| serial.write_packet(&data[..to_send])) {
                    Ok(size) => {
                        if size < data.len() {
                            data.drain(..size);
                            tx_data.replace(data);

                            wait = short_wait; // fast response
                        }
                    }
                    Err(usb_device::UsbError::WouldBlock) => {
                        tx_data.replace(data);
                        wait = short_wait; // fast response
                    }
                    Err(e) => {
                        defmt::error!("Failed to send data over USB: {}", defmt::Debug2Format(&e));
                    }
                }
            } else if let Ok(data) = protobuf_output_rx.try_recv() {
                tx_data.replace(data);

                wait = short_wait; // fast response
            }
        }
    }

    #[task(shared = [output_storage, settings], local = [protobuf_input_rx, protobuf_output_tx, storage_meta], priority = 2)]
    async fn protobuf_server(ctx: protobuf_server::Context) {
        let mut rx_stream = impls::AsyncProtobufStream::new(ctx.local.protobuf_input_rx);
        let mut output_storage = ctx.shared.output_storage;
        let mut settings = ctx.shared.settings;
        let protobuf_output_tx = ctx.local.protobuf_output_tx;
        let storage_meta = *ctx.local.storage_meta;

        let mut get_output = move || output_storage.lock(|storage| storage.clone());
        let mut with_settings = move |f: &mut dyn FnMut(
            &mut (settings::AppSettings, settings::NonStoreSettings),
        ) -> (bool, bool)| {
            let mut s: (settings::AppSettings, settings::NonStoreSettings) =
                settings.lock(|settings| {
                    let s = settings.ref_mut();
                    (s.0.clone(), s.1.clone())
                });

            let (modified, save) = f(&mut s);

            if modified {
                settings.lock(move |settings| {
                    let rs = settings.ref_mut();
                    *rs.0 = s.0;
                    *rs.1 = s.1;
                });
            }

            save
        };

        loop {
            match impls::process_protobuf(
                &mut rx_stream,
                protobuf_output_tx,
                || Mono::now().ticks(),
                &mut get_output,
                &mut with_settings,
                storage_meta,
            )
            .await
            {
                Err(e) => {
                    defmt::error!("Protobuf error: {}", e);
                }
                Ok(true) => {
                    if let Err(_) = settings_saver::spawn() {
                        defmt::error!("Failed to spawn settings_saver task");
                    }
                }
                _ => (),
            }
        }
    }

    #[task(shared = [output_storage, &base_period, analog_sens], priority = 2)]
    async fn read_analog(ctx: read_analog::Context) {
        let base_period = *ctx.shared.base_period;

        let mut analog_sens = ctx.shared.analog_sens;
        let mut output_storage = ctx.shared.output_storage;

        defmt::info!("Regular test task");
        loop {
            let (vbat, tcpu, v_bat_raw, v_tewmp_raw) = analog_sens.lock(|sens| sens.read());
            defmt::trace!(
                "Analog read: vbat = {} V, tcpu = {} °C, v_bat_raw = {}, t_cpu_raw = {}",
                vbat,
                tcpu,
                v_bat_raw,
                v_tewmp_raw
            );

            output_storage
                .lock(move |storage| storage.set_analog_values(vbat, tcpu, v_bat_raw, v_tewmp_raw));

            Mono::delay(base_period).await;
        }
    }

    #[task(
        shared = [
            &base_period,
            &rtc_sync,
            &f1_base_period_devider,
            &f2_base_period_devider,
            output_storage
        ],
        local = [
            self_writer_data_tx
        ],
        priority = 4
    )]
    async fn self_writer_data_catcher(ctx: self_writer_data_catcher::Context) {
        let base_period = *ctx.shared.base_period;
        let rtc_sync = ctx.shared.rtc_sync;
        let f1_base_period_devider = *ctx.shared.f1_base_period_devider;
        let f2_base_period_devider = *ctx.shared.f2_base_period_devider;
        let self_writer_data_tx = ctx.local.self_writer_data_tx;

        let mut output_storage = ctx.shared.output_storage;

        let mut channel_selector = FChannel::iter(f1_base_period_devider, f2_base_period_devider);

        rtc_sync
            .delay_sync(config::Duration::millis(
                config::PREHEAT_MIN_MS
                    + config::MEASURE_TIME_MAX_MS
                    + config::BASE_INTERVAL_MIN_MS
                    + (1_000 / config::SYST_TIMER_HZ),
            ))
            .await;

        loop {
            rtc_sync.delay_sync(base_period).await;

            match channel_selector.next() {
                Some(FChannel::Both) => {
                    let (f1, f2) = output_storage.lock(|storage| {
                        (
                            storage.frequencys[FChannel::Pressure as usize],
                            storage.frequencys[FChannel::Temperature as usize],
                        )
                    });

                    if self_writer_data_tx
                        .try_send(types::FData::Both(f1.unwrap_or(0.0), f2.unwrap_or(0.0)))
                        .is_err()
                    {
                        defmt::error!("Failed to send Both data");
                    }
                }
                Some(FChannel::Pressure) => {
                    let f = output_storage
                        .lock(|storage| storage.frequencys[FChannel::Pressure as usize]);

                    if self_writer_data_tx
                        .try_send(types::FData::Pressure(f.unwrap_or(0.0)))
                        .is_err()
                    {
                        defmt::error!("Failed to send Pressure data");
                    }
                }
                Some(FChannel::Temperature) => {
                    let f = output_storage
                        .lock(|storage| storage.frequencys[FChannel::Temperature as usize]);
                    if self_writer_data_tx
                        .try_send(types::FData::Temperature(f.unwrap_or(0.0)))
                        .is_err()
                    {
                        defmt::error!("Failed to send Temperature data");
                    }
                }
                None => (),
            }
        }
    }

    #[task(
        shared = [rtc, led, analog_sens, settings],
        local = [self_writer_data_rx, storage_context, crc_handler],
        priority = 1
    )]
    async fn self_writer_packer(ctx: self_writer_packer::Context) {
        let mut analog_sens = ctx.shared.analog_sens;
        let mut settings = ctx.shared.settings;
        let mut rtc = ctx.shared.rtc;
        let mut led = ctx.shared.led;

        let self_writer_data_rx = ctx.local.self_writer_data_rx;
        let storage_context = ctx.local.storage_context.as_mut().unwrap();
        let crc_handler = ctx.local.crc_handler;

        let mut block_id = 0; // Всегда начинаем цепочку блоков с 0
        let write_config = settings.lock(|settings| settings.ref_mut().0.write_config);

        let mut prevs = [0i32; 2];
        let mut current_block_packer = Option::<DataBlockPacker>::None;

        let mut finalize_block = move |packer: DataBlockPacker| {
            packer.to_result_full(|data| {
                crc_handler.reset();
                crc_handler.feed(data);
                crc_handler.result()
            })
        };

        let mut await_both = true; // первая запись в блоке длжна быть FChannel::Both

        let calc_diff = |value: f64, prev: &mut i32| {
            let f_fixed = (value * config::FREQ_MULTIPLIER as f64) as i32;

            let diff = f_fixed - *prev;
            *prev = f_fixed;

            diff
        };

        fn push_val(packer: &mut DataBlockPacker, diff: i32) -> bool {
            match packer.push_val(diff) {
                PushResult::Success => false,
                PushResult::Full => true,
                PushResult::Overflow => {
                    impls::halt_device("Self-writer block packer overflowed");
                }
                PushResult::Finished => {
                    impls::halt_device("Self-writer packer entered invalid finished state");
                }
            }
        }

        loop {
            if current_block_packer.is_none() {
                let used_blocks = storage_context.used_blocks();
                if used_blocks >= storage_context.total_blocks() {
                    impls::halt_device("Self-writer storage is full");
                }

                let analog_values = analog_sens.lock(|sens| sens.read());

                // блок с id=0 не имеет предыдущего блока, по этому 0
                let prev_block_id = if block_id > 0 { block_id - 1 } else { 0 };

                prevs = [0i32; 2];
                await_both = true;

                current_block_packer = Some(
                    DataBlockPacker::builder()
                        .set_ids(prev_block_id, block_id)
                        .set_timestamp(rtc.lock(|rtc| rtc.current_time()).into())
                        .set_write_cfg(
                            write_config.base_interval_ms,
                            [write_config.p_write_devider, write_config.t_write_devider],
                        )
                        .set_tcpu(analog_values.1)
                        .set_vbat(analog_values.0)
                        .set_size(config::STORAGE_BLOCK_SIZE_BYTES as usize)
                        .build(),
                );
            }

            let packer = current_block_packer.as_mut().unwrap();

            let item = {
                loop {
                    let item = self_writer_data_rx
                        .recv()
                        .await
                        .expect("Failed to receive data from self_writer_data_catcher task");
                    if await_both && (item.as_channel() != FChannel::Both) {
                        defmt::warn!(
                            "Awaiting for FChannel::Both data, but received {} data",
                            item.as_channel()
                        );
                    } else {
                        await_both = false;
                        break item;
                    }
                }
            };

            let is_full = match item {
                types::FData::Pressure(fp) => {
                    let diff = calc_diff(fp, &mut prevs[item.as_channel() as usize]);
                    push_val(packer, diff)
                }
                types::FData::Temperature(ft) => {
                    let diff = calc_diff(ft, &mut prevs[item.as_channel() as usize]);
                    push_val(packer, diff)
                }
                types::FData::Both(fp, ft) => {
                    let diff_p = calc_diff(fp, &mut prevs[FChannel::Pressure as usize]);
                    if !push_val(packer, diff_p) {
                        let diff_t = calc_diff(ft, &mut prevs[FChannel::Temperature as usize]);
                        push_val(packer, diff_t)
                    } else {
                        true
                    }
                }
            };

            if is_full {
                let packer = current_block_packer.take().unwrap();
                let data = finalize_block(packer).unwrap_or_else(|| {
                    impls::halt_device("Failed to finalize block");
                });

                #[cfg(feature = "led-blink-each-block")]
                led.lock(|led| led.set_state(config::LED_ENABLE));

                match storage_context.write_next_block(data.as_slice()) {
                    Ok(abs_id) => {
                        defmt::info!("Self-writer block {} stored (abs={})", block_id, abs_id);
                    }
                    Err(_) => {
                        impls::halt_device("Self-writer storage write failed");
                    }
                }

                #[cfg(feature = "led-blink-each-block")]
                led.lock(|led| led.set_state(config::LED_DISABLE));

                if storage_context.used_blocks() >= storage_context.total_blocks() {
                    impls::halt_device("Self-writer storage is full");
                } else {
                    block_id += 1;
                }
            }
        }
    }

    #[task(shared = [led], local = [prepare_delay], priority = 2)]
    async fn self_writer_signal(ctx: self_writer_signal::Context) {
        let mut led = ctx.shared.led;
        let mut prepare_delay = *ctx.local.prepare_delay;

        defmt::info!("+ Startup Signal +");
        for _ in 0..config::START_BLINK_COUNT {
            led.lock(|led| led.set_state(config::LED_ENABLE));
            Mono::delay(config::Duration::millis(config::START_BLINK_PERIOD_MS / 2)).await;
            led.lock(|led| led.set_state(config::LED_DISABLE));
            Mono::delay(config::Duration::millis(config::START_BLINK_PERIOD_MS / 2)).await;
        }

        if prepare_delay
            > config::Duration::millis(config::START_BLINK_PERIOD_MS * config::START_BLINK_COUNT)
        {
            prepare_delay -=
                config::Duration::millis(config::START_BLINK_PERIOD_MS * config::START_BLINK_COUNT);
            defmt::info!(
                "Startup signal done, measuring will start after {} seconds",
                prepare_delay.to_secs()
            );
            Mono::delay(prepare_delay).await;
        }

        defmt::info!("Starting measurements...");
        sync_freqmeter1::spawn().expect("Failed to spawn sync_freqmeter1 task");
        sync_freqmeter2::spawn().expect("Failed to spawn sync_freqmeter2 task");
        self_writer_data_catcher::spawn().expect("Failed to spawn self_writer task");
        self_writer_packer::spawn().expect("Failed to spawn self_writer_packer task");
    }

    #[task(local = [storage_erase], priority = 1)]
    async fn flash_erase(ctx: flash_erase::Context) {
        let storage_erase = *ctx.local.storage_erase;
        match storage_erase.process_pending_erase() {
            Ok(true) => defmt::info!("Storage erase completed"),
            Ok(false) => (),
            Err(e) => defmt::error!("Storage erase failed: {}", defmt::Debug2Format(&e)),
        }
    }
}
