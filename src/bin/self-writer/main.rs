#![no_std]
#![no_main]
// For allocator
#![feature(alloc_error_handler)]
#![feature(adt_const_params)]

mod types;

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

define_self_writer_monotonic!(SelfWriterMonotonicSlow, {
    config::SYST_TIMER_HZ_SELF_WRITER_MODE
});
define_self_writer_monotonic!(SelfWriterMonotonicFast, {
    config::SYST_TIMER_HZ_HIGH_FREQ_MODE
});

//-----------------------------------------------------------------------------

static mut HEAP: [u8; config::HEAP_SIZE] = [0; config::HEAP_SIZE];

//-----------------------------------------------------------------------------

#[app(device = stm32l4xx_hal::pac, peripherals = true, dispatchers = [RTC_ALARM, LCD])]
mod app {
    use stm32l4xx_hal::gpio;

    use super::*;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        led: types::Led,
        flash1: types::Flash1,
        flash_reset_pin: types::FlashResetPin,
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
        if high_perf_mode {
            SelfWriterMonotonicFast::start(ctx.core.SYST, clocks.sysclk().0);
        } else {
            SelfWriterMonotonicSlow::start(ctx.core.SYST, clocks.sysclk().0);
        }
        defmt::info!("\tSysTick");

        let mut gpioa = ctx.device.GPIOA.split(&mut rcc.ahb2);
        let mut gpiob = ctx.device.GPIOB.split(&mut rcc.ahb2);
        let mut gpioc = ctx.device.GPIOC.split(&mut rcc.ahb2);
        let mut gpiod = ctx.device.GPIOD.split(&mut rcc.ahb2);
        let mut gpioe = ctx.device.GPIOE.split(&mut rcc.ahb2);

        let (flash1, flash_reset_pin) = {
            #[cfg(feature = "no-flash")]
            {
                ((), ())
            }

            #[cfg(not(feature = "no-flash"))]
            {
                #[cfg(feature = "maket")]
                let io_0 =
                    gpioe
                        .pe12
                        .into_alternate(&mut gpioe.moder, &mut gpioe.otyper, &mut gpioe.afrh);
                #[cfg(not(feature = "maket"))]
                let io_0 =
                    gpiob
                        .pb1
                        .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);

                stm32_usb_self_writer::workmodes::common::create_qspi(
                    (
                        gpioa.pa3.into_alternate(
                            &mut gpioa.moder,
                            &mut gpioa.otyper,
                            &mut gpioa.afrl,
                        ),
                        gpioa.pa2.into_alternate(
                            &mut gpioa.moder,
                            &mut gpioa.otyper,
                            &mut gpioa.afrl,
                        ),
                        io_0,
                        gpiob.pb0.into_alternate(
                            &mut gpiob.moder,
                            &mut gpiob.otyper,
                            &mut gpiob.afrl,
                        ),
                        gpioa.pa7.into_alternate(
                            &mut gpioa.moder,
                            &mut gpioa.otyper,
                            &mut gpioa.afrl,
                        ),
                        gpioa.pa6.into_alternate(
                            &mut gpioa.moder,
                            &mut gpioa.otyper,
                            &mut gpioa.afrl,
                        ),
                    ),
                    gpiod.pd11.into_push_pull_output_in_state(
                        &mut gpiod.moder,
                        &mut gpiod.otyper,
                        PinState::Low,
                    ),
                    &mut rcc.ahb3,
                )
            }
        };
        defmt::info!("\tQSPI");

        let led = gpioc.pc10.into_push_pull_output_in_state(
            &mut gpioc.moder,
            &mut gpioc.otyper,
            config::LED_DISABLE,
        );
        defmt::info!("\tLED");

        (
            Shared {},
            Local {
                led,
                flash1,
                flash_reset_pin,
            },
        )
    }
}
