#![no_std]
#![no_main]
// For allocator
#![feature(alloc_error_handler)]
#![feature(adt_const_params)]

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
    use super::*;

    #[shared]
    struct Shared {
        rtc: RtcService,
    }

    #[local]
    struct Local {
        led: types::Led,

        analog_sens: stm32_usb_self_writer::sensors::analog::AnalogSensor<types::VBatPin>,
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

        let (clocks, high_perf_mode) = if fast_mode {
            defmt::info!("\tUSB connected, starting in high performance mode");
            (
                types::HighPerformanceClockProvider::configure_clocks(
                    &mut flash, &mut rcc, &mut pwr,
                ),
                true,
            )
        } else {
            defmt::info!("\tUSB not connected, starting in recorder mode");
            (
                types::RecorderClockProvider::configure_clocks(&mut flash, &mut rcc, &mut pwr),
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

        (Shared { rtc }, Local { led, analog_sens })
    }

    //-------------------------------------------------------------------------

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
