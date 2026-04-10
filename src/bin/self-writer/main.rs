#![no_std]
#![no_main]

mod init;
mod types;

extern crate alloc;

use defmt_rtt as _; // global logger
use panic_abort as _;

use stm32l4xx_hal::{
    dma::dma1,
    gpio::{Output, PD10, PD13, PushPull},
    pac::{TIM1, TIM2},
    prelude::*,
};

use rtic::app;
use rtic_monotonics::{Monotonic, fugit::ExtU32};
use rtic_sync::channel::{Receiver, Sender};

use stm32_usb_self_writer::{
    InputChannel, RtcSync,
    clocking::rtc::RtcService,
    config, is_usb_connected,
    sensors::freqmeter::{Capture, Capturer, ExtInputType, TimerInpitCounterExt},
    settings,
    support::crc::STM32L4Crc32,
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

//-----------------------------------------------------------------------------

#[app(device = stm32l4xx_hal::pac, peripherals = true, dispatchers = [RCC, LCD])]
mod app {
    use super::*;

    #[shared]
    struct Shared {
        rtc: RtcService,
        base_period: config::Duration,
        start_delay: config::Duration,
        f1_base_period_devider: u32,
        f2_base_period_devider: u32,
        rtc_sync: RtcSync<Mono>,

        master_counter_freq: stm32l4xx_hal::time::Hertz,

        transfer_fin1: dma1::C6,
        f1_capturer: Capturer<TIM1, { ExtInputType::TI1FP1 as u8 }>,

        transfer_fin2: dma1::C2,
        f2_capturer: Capturer<TIM2, { ExtInputType::TI1FP1 as u8 }>,

        settings: settings::SettingsManagerType,
        flash_policy: settings::FlasRWPolcy<settings::AppSettings, STM32L4Crc32>,
    }

    #[local]
    struct Local {
        led: types::Led,

        analog_sens: stm32_usb_self_writer::sensors::analog::AnalogSensor<types::VBatPin>,

        master_timer: types::MasterCounter,
        f1_power_pin: PD13<Output<PushPull>>,
        f2_power_pin: PD10<Output<PushPull>>,

        f1_capture_buffer: &'static mut types::MasterCounterType,
        f1_capture_tx: Sender<'static, Capture, 1>,
        f1_capture_rx: Receiver<'static, Capture, 1>,

        f2_capture_buffer: &'static mut types::MasterCounterType,
        f2_capture_tx: Sender<'static, Capture, 1>,
        f2_capture_rx: Receiver<'static, Capture, 1>,
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
        Mono::start(ctx.core.SYST, clocks.hclk().0);
        defmt::info!("\tSysTick");

        let (settings, flash_policy, base_period, start_delay, write_config) =
            init_settings(flash, dp.CRC, &mut rcc, high_perf_mode);

        let rtc = init_rtc_service(
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
        let mut gpioe = dp.GPIOE.split(&mut rcc.ahb2);

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
        let f1_power_pin = gpiod.pd13.into_push_pull_output_in_state(
            &mut gpiod.moder,
            &mut gpiod.otyper,
            config::GENERATOR_DISABLE_LVL,
        );
        defmt::info!("\tFreqmeter 1");

        let (transfer_fin2, f2_capturer, f2_capture_buffer, (f2_capture_tx, f2_capture_rx)) = stm32_usb_self_writer::build_freqmeter_dma!(
            input_timer = dp
                .TIM2
                .into_input_counter(gpioa.pa0.into_alternate_push_pull(
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
        let f2_power_pin = gpiod.pd10.into_push_pull_output_in_state(
            &mut gpiod.moder,
            &mut gpiod.otyper,
            config::GENERATOR_DISABLE_LVL,
        );
        defmt::info!("\tFreqmeter 2");

        //{
        //    let usbperith = UsbPeriph {
        //        usb: dp.USB,
        //        pin_dm: gpioa
        //            .pa11
        //            .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh)
        //            .set_speed(Speed::VeryHigh),
        //        pin_dp: gpioa
        //            .pa12
        //            .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh)
        //            .set_speed(Speed::VeryHigh),
        //    };
        //}

        let led = gpioc.pc10.into_push_pull_output_in_state(
            &mut gpioc.moder,
            &mut gpioc.otyper,
            config::LED_DISABLE,
        );
        defmt::info!("\tLED");

        //---------------------------------------------------------------------

        sync_freqmeter1::spawn().expect("Failed to spawn sync_freqmeter1 task");
        sync_freqmeter2::spawn().expect("Failed to spawn sync_freqmeter2 task");

        //regular_test::spawn().expect("Failed to spawn regular test task");

        defmt::info!("Tasks spawned");

        //---------------------------------------------------------------------

        (
            Shared {
                rtc,
                base_period,
                start_delay,
                f1_base_period_devider: write_config.p_write_devider,
                f2_base_period_devider: write_config.t_write_devider,
                rtc_sync: RtcSync::<Mono>::new(base_period),

                master_counter_freq,

                transfer_fin1,
                f1_capturer,

                transfer_fin2,
                f2_capturer,

                settings,
                flash_policy,
            },
            Local {
                led,
                analog_sens,
                master_timer,

                f1_power_pin,
                f2_power_pin,

                f1_capture_buffer,
                f1_capture_tx,
                f1_capture_rx,
                f2_capture_buffer,
                f2_capture_tx,
                f2_capture_rx,
            },
        )
    }

    //-------------------------------------------------------------------------

    #[task(binds = TIM6_DAC, local = [master_timer], priority = 6)]
    fn master_timer_ovf(ctx: master_timer_ovf::Context) {
        unsafe { ctx.local.master_timer.overflow_isr() };
    }

    #[task(
        binds=DMA1_CH6,
        shared = [transfer_fin1, f1_capturer],
        local = [f1_capture_buffer, f1_capture_tx],
        priority = 4)
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
        binds=DMA1_CH2,
        shared = [transfer_fin2, f2_capturer],
        local = [f2_capture_buffer, f2_capture_tx],
        priority = 4)
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

    // Приоритет строго равен sync_freqmeter*, иначе Deadlock на мьютексе rtc_sync
    #[task(binds = RTC_WKUP, shared = [rtc, &rtc_sync], priority = 2)]
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
            &start_delay,
        ],
        local = [f1_capture_rx, f1_power_pin],
        priority = 2,
    )]
    async fn sync_freqmeter1(ctx: sync_freqmeter1::Context) {
        stm32_usb_self_writer::freqmeter!(
            channel = InputChannel::Ch1,
            start_delay = *ctx.shared.start_delay,
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
            &start_delay,
        ],
        local = [f2_capture_rx, f2_power_pin],
        priority = 2,
    )]
    async fn sync_freqmeter2(ctx: sync_freqmeter2::Context) {
        stm32_usb_self_writer::freqmeter!(
            channel = InputChannel::Ch2,
            start_delay = *ctx.shared.start_delay,
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

    #[task(
        shared = [settings, flash_policy],
        priority = 1,
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

        // reset device
        cortex_m::peripheral::SCB::sys_reset();
    }

    #[task(local = [analog_sens], priority = 1)]
    async fn regular_test(ctx: regular_test::Context) {
        let analog_sens = ctx.local.analog_sens;

        defmt::info!("Regular test task");
        loop {
            let (vbat, tcpu) = analog_sens.read();
            defmt::info!("VBAT: {} V, TCPU: {} °C", vbat, tcpu);

            Mono::delay(1000u32.millis()).await;
        }
    }
}
