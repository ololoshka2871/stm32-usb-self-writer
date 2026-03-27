#![no_std]
#![no_main]
// For allocator
#![feature(alloc_error_handler)]
#![feature(adt_const_params)]

extern crate alloc;

use defmt_rtt as _; // global logger
use panic_abort as _;

use cortex_m_rt::entry;
use stm32l4xx_hal::stm32;

use rtic_monotonics::fugit::RateExtU32;
use stm32l4xx_hal::flash::FlashExt;
use stm32l4xx_hal::prelude::*;

use rtic::app;

use stm32_usb_self_writer::{
    config, define_self_writer_monotonic, is_usb_connected, start_at_mode,
    workmodes::common::ClockConfigProvider, FreeRtosErrorContainer, HighPerformanceMode,
    RecorderMode,
};

//---------------------------------------------------------------

#[global_allocator]
static GLOBAL: freertos_rust::FreeRtosAllocator = freertos_rust::FreeRtosAllocator;

//-----------------------------------------------------------------------------

define_self_writer_monotonic!(SelfWriterMonotonicSlow, {
    config::SYST_TIMER_HZ_SELF_WRITER_MODE
});
define_self_writer_monotonic!(SelfWriterMonotonicFast, {
    config::SYST_TIMER_HZ_HIGH_FREQ_MODE
});

//-----------------------------------------------------------------------------

#[app(device = stm32l4xx_hal::pac, peripherals = true, dispatchers = [RTC_ALARM, LCD])]
mod app {
    use super::*;

    struct Pll;

    #[cfg(feature = "xtal-24mhz")]
    impl stm32_usb_self_writer::workmodes::high_performance_mode::PllConfigProvider for Pll {
        const PD: u32 = 3;
        const M: u32 = 20;
        const AD: u32 = 2;

        const SAI_MUL: u32 = 12;
        const SAI_DIV_CODE: u32 = 2;
    }

    #[cfg(feature = "xtal-12mhz")]
    impl stm32_usb_self_writer::workmodes::high_performance_mode::PllConfigProvider for Pll {
        const PD: u32 = 3;
        const M: u32 = 40;
        const AD: u32 = 2;

        const SAI_MUL: u32 = 24;
        const SAI_DIV_CODE: u32 = 2;
    }

    type HighPerformanceClockProvider =
        stm32_usb_self_writer::workmodes::high_performance_mode::HighPerformanceClockConfigProvider<
            Pll,
            { config::XTAL_FREQ },
            { config::SELF_WRITER_CPU_FREQ },
        >;
    type RecorderClockProvider =
        stm32_usb_self_writer::workmodes::recorder_mode::RecorderClockConfigProvider<
            { config::XTAL_FREQ },
            { config::SELF_WRITER_CPU_FREQ },
        >;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {}

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

        let clocks = if fast_mode {
            HighPerformanceClockProvider::configure_clocks(&mut flash, &mut rcc, &mut pwr)
        } else {
            RecorderClockProvider::configure_clocks(&mut flash, &mut rcc, &mut pwr)
        };
        defmt::info!("\tClocks: {}", defmt::Debug2Format(&clocks));

        (Shared {}, Local {})
    }
}
