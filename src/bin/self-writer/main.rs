#![no_std]
#![no_main]

mod types;

extern crate alloc;

use defmt_rtt as _; // global logger
use panic_abort as _;

use stm32l4xx_hal::prelude::*;

use rtic::app;
use rtic_monotonics::{fugit::ExtU64, systick_monotonic, Monotonic};

use stm32_usb_self_writer::{
    clocking::{rtc::RtcService, ClockConfigProvider},
    config, is_usb_connected,
    sensors::freqmeter::TimerInpitCounterExt,
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
    }

    #[local]
    struct Local {
        led: types::Led,

        analog_sens: stm32_usb_self_writer::sensors::analog::AnalogSensor<types::VBatPin>,

        master_timer: types::MasterCounter,
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

        let xtal_clocks = config::XTAL_FREQ;

        let (clocks, master_counter_freq, high_perf_mode) = if fast_mode {
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
        let master_timer = types::MasterCounter::new(stm32l4xx_hal::timer::Timer::tim6(
            dp.TIM6,
            1.hz(),
            clocks,
            &mut rcc.apb1r1,
        ));

        let dma1 = dp.DMA1.split(&mut rcc.ahb1);

        let (
            freqmeter1,
            //transfer_fin1,
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
            dp = dp,
            stop_reg = apb2_fz,
            stop_bit = dbg_tim1_stop
        );

        let (
            freqmeter2,
            //transfer_fin2,
            f2_capturer,
            f2_capture_buffer,
            (f2_capture_tx, f2_capture_rx),
            (f2_target_tx, f2_target_rx),
        ) = stm32_usb_self_writer::build_freqmeter!(
            input_timer = dp
                .TIM2
                .into_input_counter(gpioa.pa5.into_alternate_push_pull(
                    &mut gpioa.moder,
                    &mut gpioa.otyper,
                    &mut gpioa.afrl
                )),
            dma_channel = dma.2, // DMA1 Channel 2[CxS=4] is connected to TIM2_UP
            master_timer = master_timer,
            master_type = types::MasterCounterType,
            dp = dp,
            stop_reg = apb1_fz,
            stop_bit = dbg_tim2_stop
        );

        let led = gpioc.pc10.into_push_pull_output_in_state(
            &mut gpioc.moder,
            &mut gpioc.otyper,
            config::LED_DISABLE,
        );
        defmt::info!("\tLED");

        //---------------------------------------------------------------------

        regular_test::spawn().expect("Failed to spawn regular test task");

        defmt::info!("Tasks spawned");

        //---------------------------------------------------------------------

        (
            Shared { rtc },
            Local {
                led,
                analog_sens,
                master_timer,
            },
        )
    }

    //-------------------------------------------------------------------------

    #[task(binds = TIM6_DAC, local = [master_timer], priority = 6)]
    fn master_timer_ovf(ctx: master_timer_ovf::Context) {
        unsafe { ctx.local.master_timer.overflow() };
    }

    //#[task(binds=DMA1_CH4_5_6_7, shared = [transfer_fin1, f1_capturer], local = [f1_capture_buffer, f1_capture_tx, f1_target_rx], priority = 3)]
    //fn f1_dma_transfer_complete(mut ctx: f1_dma_transfer_complete::Context) {
    //    dma_interrupt!(
    //        buffer: **ctx.local.f1_capture_buffer,
    //        cature_tx: ctx.local.f1_capture_tx,
    //        target_rx: ctx.local.f1_target_rx,
    //        capturerer: ctx.shared.f1_capturer,
    //        transfer: ctx.shared.transfer_fin1,
    //        cgifX: cgif5
    //    );
    //}
    //
    //#[task(binds=DMA1_CH2_3, shared = [transfer_fin2, f2_capturer], local = [f2_capture_buffer, f2_capture_tx, f2_target_rx], priority = 3)]
    //fn f2_dma_transfer_complete(mut ctx: f2_dma_transfer_complete::Context) {
    //    dma_interrupt!(
    //        buffer: **ctx.local.f2_capture_buffer,
    //        cature_tx: ctx.local.f2_capture_tx,
    //        target_rx: ctx.local.f2_target_rx,
    //        capturerer: ctx.shared.f2_capturer,
    //        transfer: ctx.shared.transfer_fin2,
    //        cgifX: cgif3
    //    );
    //}

    #[task(binds = RTC_WKUP, shared = [rtc], priority = 1)]
    fn rtc_alarm(ctx: rtc_alarm::Context) {
        let mut rtc = ctx.shared.rtc;

        rtc.lock(|rtc| rtc.handle_alarm_interrupt());

        let now = rtc.lock(|rtc| rtc.current_time());
        defmt::info!("RTC Alarm! Current time: {}", now);
    }

    //-------------------------------------------------------------------------

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
