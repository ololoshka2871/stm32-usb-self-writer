#![no_std]
#![no_main]

mod types;

extern crate alloc;

use defmt_rtt as _; // global logger
use panic_abort as _;

use stm32l4xx_hal::stm32;

use rtic_monotonics::fugit::RateExtU32;
use stm32l4xx_hal::flash::FlashExt;
use stm32l4xx_hal::prelude::*;

use rtic::app;
use rtic_monotonics::{fugit::ExtU64, systick_monotonic, Monotonic};

use stm32_usb_self_writer::{
    clocking::{rtc::RtcService, ClockConfigProvider, PllConfigProvider},
    config, is_usb_connected,
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
    use stm32_usb_self_writer::sensors::freqmeter::{self, FreqmetersScaffold};

    use super::*;

    #[shared]
    struct Shared {
        rtc: RtcService,
    }

    #[local]
    struct Local {
        led: types::Led,

        analog_sens: stm32_usb_self_writer::sensors::analog::AnalogSensor<types::VBatPin>,
        _freqmeters: FreqmetersScaffold,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local) {
        #[cfg(feature = "force-defmt-logs")]
        // need for defmt logging works https://github.com/knurling-rs/probe-run/pull/183/files
        ctx.device.RCC.ahb1enr.modify(|_, w| w.dma1en().set_bit());

        defmt::info!("+ Init +");

        ctx.core.DCB.enable_trace();
        ctx.core.DWT.enable_cycle_counter();
        defmt::info!("\tDWT");

        let fast_mode = is_usb_connected();

        let mut flash = ctx.device.FLASH.constrain();
        let mut rcc = ctx.device.RCC.constrain();
        let mut pwr = ctx.device.PWR.constrain(&mut rcc.apb1r1);

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
            ctx.device.RTC,
            &mut ctx.device.EXTI,
            &mut rcc.apb1r1,
            &mut rcc.bdcr,
            &mut pwr.cr1,
        );
        rtc.set_alarm_period_ms(1_000);
        defmt::info!(
            "\tRTC initialized, source: {}",
            defmt::Debug2Format(&rtc_clock_source)
        );

        let mut gpioa = ctx.device.GPIOA.split(&mut rcc.ahb2);
        let mut gpiob = ctx.device.GPIOB.split(&mut rcc.ahb2);
        let mut gpioc = ctx.device.GPIOC.split(&mut rcc.ahb2);
        let mut gpiod = ctx.device.GPIOD.split(&mut rcc.ahb2);
        let mut gpioe = ctx.device.GPIOE.split(&mut rcc.ahb2);

        let analog_sens = {
            let mut delay = stm32_usb_self_writer::NOPDelay {
                sys_clk: clocks.sysclk(),
            };

            let mut adc = stm32l4xx_hal::adc::ADC::new(
                ctx.device.ADC1,
                ctx.device.ADC_COMMON,
                &mut rcc.ahb2,
                &mut rcc.ccipr,
                &mut delay,
            );

            let vbat_pin = gpioa.pa1.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);

            stm32_usb_self_writer::sensors::analog::AnalogSensor::new(adc, vbat_pin, &mut delay)
        };
        defmt::info!("\tAnalog sensor");

        let freqmeters = {
            // TODO: build_freqmeter!
        };

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
                _freqmeters: freqmeters,
            },
        )
    }

    //-------------------------------------------------------------------------

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
