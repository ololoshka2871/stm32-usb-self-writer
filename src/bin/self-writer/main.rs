#![no_std]
#![no_main]

mod types;

extern crate alloc;

use defmt_rtt as _; // global logger
use panic_abort as _;

use stm32l4xx_hal::{
    dma::dma1,
    pac::{TIM1, TIM2},
    prelude::*,
    gpio::{PD10, PD13, Output, PushPull},
};

use rtic::app;
use rtic_monotonics::{fugit::ExtU64, systick_monotonic, Monotonic};
use rtic_sync::channel::{Receiver, Sender};

use stm32_usb_self_writer::{
    clocking::{rtc::RtcService, ClockConfigProvider},
    config, is_usb_connected,
    sensors::freqmeter::{Capture, Capturer, ExtInputType, Freqmeter, TimerInpitCounterExt},
};

//-----------------------------------------------------------------------------

systick_monotonic!(Mono, config::SYST_TIMER_HZ);

//-----------------------------------------------------------------------------

defmt::timestamp!(
    "[T{=u64:ms}]",
    Mono::now().ticks() * (1_000 / config::SYST_TIMER_HZ as u64)
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

        master_counter_freq: stm32l4xx_hal::time::Hertz,

        transfer_fin1: dma1::C6,
        f1_capturer: Capturer<TIM1, { ExtInputType::TI1FP1 as u8 }>,

        transfer_fin2: dma1::C2,
        f2_capturer: Capturer<TIM2, { ExtInputType::TI1FP1 as u8 }>,
    }

    #[local]
    struct Local {
        led: types::Led,

        analog_sens: stm32_usb_self_writer::sensors::analog::AnalogSensor<types::VBatPin>,

        master_timer: types::MasterCounter,
        freqmeter1: Freqmeter<PD13<Output<PushPull>>, { config::SYST_TIMER_HZ }>,
        freqmeter2: Freqmeter<PD10<Output<PushPull>>, { config::SYST_TIMER_HZ }>,

        f1_capture_buffer: &'static mut types::MasterCounterType,
        f1_capture_tx: Sender<'static, Capture, 1>,
        f1_capture_rx: Receiver<'static, Capture, 1>,

        f2_capture_buffer: &'static mut types::MasterCounterType,
        f2_capture_tx: Sender<'static, Capture, 1>,
        f2_capture_rx: Receiver<'static, Capture, 1>,

        f1_target_tx: Sender<'static, u16, 1>,
        f1_target_rx: Receiver<'static, u16, 1>,  
        f2_target_tx: Sender<'static, u16, 1>,
        f2_target_rx: Receiver<'static, u16, 1>,
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

        let (clocks, master_counter_freq, _high_perf_mode) = if fast_mode {
            defmt::info!("\tUSB connected, starting in high performance mode");
            (
                types::HighPerformanceClockProvider::configure_clocks(
                    &mut flash, &mut rcc, &mut pwr,
                ),
                types::HighPerformanceClockProvider::master_counter_frequency(),
                true,
            )
        } else {
            defmt::info!("\tUSB not connected, starting in recorder mode");
            (
                types::RecorderClockProvider::configure_clocks(&mut flash, &mut rcc, &mut pwr),
                types::RecorderClockProvider::master_counter_frequency(),
                false,
            )
        };
        defmt::info!("\tClocks: {}", defmt::Debug2Format(&clocks));

        unsafe {
            #[allow(static_mut_refs)]
            umm_malloc::init_heap(HEAP.as_mut_ptr() as usize, config::HEAP_SIZE)
        };

        defmt::info!("\tHeap");

        // Initialize the systick interrupt & obtain the token to prove that we did
        Mono::start(ctx.core.SYST, clocks.hclk().0);
        defmt::info!("\tSysTick");

        let (mut rtc, rtc_clock_source) = RtcService::init(
            dp.RTC,
            &mut dp.EXTI,
            &mut rcc.apb1r1,
            &mut rcc.bdcr,
            &mut pwr.cr1,
        );
        rtc.set_alarm_period_ms(1_000);
        defmt::info!(
            "\tRTC initialized, source: {}",
            defmt::Debug2Format(&rtc_clock_source)
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

        let analog_sens = {
            let mut delay = stm32_usb_self_writer::NOPDelay {
                sys_clk: clocks.sysclk(),
            };

            let adc = stm32l4xx_hal::adc::ADC::new(
                dp.ADC1,
                dp.ADC_COMMON,
                &mut rcc.ahb2,
                &mut rcc.ccipr,
                &mut delay,
            );

            let vbat_pin = gpioa.pa1.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);

            stm32_usb_self_writer::sensors::analog::AnalogSensor::new(adc, vbat_pin, &mut delay)
        };
        defmt::info!("\tAnalog sensor");

        // Master timer
        let mut master_timer = types::MasterCounter::new(stm32l4xx_hal::timer::Timer::tim6(
            dp.TIM6,
            1.hz(),
            clocks,
            &mut rcc.apb1r1,
        ));

        master_timer.listen();
        defmt::info!("\tMaster timer");

        let dma1 = dp.DMA1.split(&mut rcc.ahb1);

        let (
            mut freqmeter1,
            transfer_fin1,
            f1_capturer,
            f1_capture_buffer,
            (f1_capture_tx, f1_capture_rx),
            (f1_target_tx, f1_target_rx),
        ) = stm32_usb_self_writer::build_freqmeter!(
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
            power_pin = gpiod.pd13.into_push_pull_output_in_state(
                &mut gpiod.moder,
                &mut gpiod.otyper,
                config::GENERATOR_DISABLE_LVL,
            ),
            dp = dp,
            stop_reg = apb2fzr,
            stop_bit = dbg_tim1_stop
        );
        freqmeter1.power_ctrl(true);
        defmt::info!("\tFreqmeter 1");

        let (
            mut freqmeter2,
            transfer_fin2,
            f2_capturer,
            f2_capture_buffer,
            (f2_capture_tx, f2_capture_rx),
            (f2_target_tx, f2_target_rx),
        ) = stm32_usb_self_writer::build_freqmeter!(
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
            power_pin = gpiod.pd10.into_push_pull_output_in_state(
                &mut gpiod.moder,
                &mut gpiod.otyper,
                config::GENERATOR_DISABLE_LVL,
            ),
            dp = dp,
            stop_reg = apb1fzr1,
            stop_bit = dbg_tim2_stop
        );
        freqmeter2.power_ctrl(true);
        defmt::info!("\tFreqmeter 2");

        let led = gpioc.pc10.into_push_pull_output_in_state(
            &mut gpioc.moder,
            &mut gpioc.otyper,
            config::LED_DISABLE,
        );
        defmt::info!("\tLED");

        //---------------------------------------------------------------------

        sync_freqmeter1::spawn().expect("Failed to spawn sync_freqmeter1 task");
        sync_freqmeter2::spawn().expect("Failed to spawn sync_freqmeter2 task");
        regular_test::spawn().expect("Failed to spawn regular test task");

        defmt::info!("Tasks spawned");

        //---------------------------------------------------------------------

        (
            Shared {
                rtc,

                master_counter_freq,

                transfer_fin1,
                f1_capturer,

                transfer_fin2,
                f2_capturer,
            },
            Local {
                led,
                analog_sens,
                master_timer,

                freqmeter1,
                freqmeter2,

                f1_capture_buffer,
                f1_capture_tx,
                f1_capture_rx,
                f2_capture_buffer,
                f2_capture_tx,
                f2_capture_rx,

                f1_target_tx,
                f1_target_rx,
                f2_target_tx,
                f2_target_rx,
            },
        )
    }

    //-------------------------------------------------------------------------

    #[task(binds = TIM6_DAC, local = [master_timer], priority = 6)]
    fn master_timer_ovf(ctx: master_timer_ovf::Context) {
        unsafe { ctx.local.master_timer.overflow() };
    }

    #[task(
        binds=DMA1_CH6, 
        shared = [transfer_fin1, f1_capturer], 
        local = [f1_capture_buffer, f1_capture_tx, f1_target_rx], 
        priority = 3)
    ]
    fn f1_dma_transfer_complete(mut ctx: f1_dma_transfer_complete::Context) {
        stm32_usb_self_writer::freqmeter_dma_interrupt!(
            buffer: **ctx.local.f1_capture_buffer,
            capture_tx: ctx.local.f1_capture_tx,
            capturer: ctx.shared.f1_capturer,
            transfer: ctx.shared.transfer_fin1,
            target_rx: ctx.local.f1_target_rx,
        );
    }

    #[task(
        binds=DMA1_CH2, 
        shared = [transfer_fin2, f2_capturer], 
        local = [f2_capture_buffer, f2_capture_tx, f2_target_rx],
        priority = 3)
    ]
    fn f2_dma_transfer_complete(mut ctx: f2_dma_transfer_complete::Context) {
        stm32_usb_self_writer::freqmeter_dma_interrupt!(
            buffer: **ctx.local.f2_capture_buffer,
            capture_tx: ctx.local.f2_capture_tx,
            capturer: ctx.shared.f2_capturer,
            transfer: ctx.shared.transfer_fin2,
            target_rx: ctx.local.f2_target_rx,
        );
    }

    #[task(binds = RTC_WKUP, shared = [rtc], priority = 1)]
    fn rtc_alarm(ctx: rtc_alarm::Context) {
        let mut rtc = ctx.shared.rtc;

        rtc.lock(|rtc| rtc.handle_alarm_interrupt());

        let now = rtc.lock(|rtc| rtc.current_time());
        defmt::info!("RTC Alarm! Current time: {}", now);
    }

    //-------------------------------------------------------------------------

    #[task(
        shared = [transfer_fin1, f1_capturer, &master_counter_freq],
        local = [f1_capture_rx, freqmeter1, f1_target_tx],
        priority = 1,
    )]
    async fn sync_freqmeter1(ctx: sync_freqmeter1::Context) {
        stm32_usb_self_writer::freqmeter!(
            channel=stm32_usb_self_writer::support::InputChannel::Ch1,
            capture_rx=ctx.local.f1_capture_rx,
            freqmeter=ctx.local.freqmeter1,
            //data_storage=(), 
            transfer_fin=ctx.shared.transfer_fin1,
            f_capturer=ctx.shared.f1_capturer,
            f_ref=*ctx.shared.master_counter_freq,
            target_tx=ctx.local.f1_target_tx,
        );
    }

    #[task(
        shared = [transfer_fin2, f2_capturer, &master_counter_freq],
        local = [f2_capture_rx, freqmeter2, f2_target_tx],
        priority = 1,
    )]
    async fn sync_freqmeter2(ctx: sync_freqmeter2::Context) {
        stm32_usb_self_writer::freqmeter!(
            channel=stm32_usb_self_writer::support::InputChannel::Ch2,
            capture_rx=ctx.local.f2_capture_rx,
            freqmeter=ctx.local.freqmeter2,
            //data_storage=(), 
            transfer_fin=ctx.shared.transfer_fin2,
            f_capturer=ctx.shared.f2_capturer,
            f_ref=*ctx.shared.master_counter_freq,
            target_tx=ctx.local.f2_target_tx,
        );
    }

    #[task(local = [analog_sens], priority = 1)]
    async fn regular_test(ctx: regular_test::Context) {
        let analog_sens = ctx.local.analog_sens;

        defmt::info!("Regular test task");
        loop {
            let (vbat, tcpu) = analog_sens.read();
            defmt::info!("VBAT: {} V, TCPU: {} °C", vbat, tcpu);

            Mono::delay(1000u64.millis()).await;
        }
    }
}
