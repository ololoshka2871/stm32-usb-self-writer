#![no_std]
#![no_main]

extern crate alloc;

use defmt_rtt as _; // global logger
use panic_abort as _;

use stm32l4xx_hal::{
    gpio::{Output, PA10, PushPull},
    pac,
    prelude::*,
};

use rtic::app;
use rtic_monotonics::Monotonic;

use stm32_usb_self_writer::{clocking::ClockConfigProvider, config};

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

#[app(device = pac, peripherals = true, dispatchers = [RCC, LCD, TAMP_STAMP, SWPMI1])]
mod app {
    use super::*;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        led: PA10<Output<PushPull>>,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local) {
        let dp = ctx.device;

        #[cfg(feature = "force-defmt-logs")]
        // need for defmt logging works https://github.com/knurling-rs/probe-run/pull/183/files
        dp.RCC.ahb1enr.modify(|_, w| w.dma1en().set_bit());

        defmt::info!("+ Init +");

        ctx.core.DCB.enable_trace();
        ctx.core.DWT.enable_cycle_counter();
        defmt::info!("\tDWT");

        unsafe {
            #[allow(static_mut_refs)]
            umm_malloc::init_heap(HEAP.as_mut_ptr() as usize, config::HEAP_SIZE)
        };

        defmt::info!("\tHeap");

        let mut flash = dp.FLASH.constrain();
        let mut rcc = dp.RCC.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);

        let clocks = stm32_usb_self_writer::clocking::RecorderClockConfigProvider::<
            { config::XTAL_FREQ },
            { config::SELF_WRITER_CPU_FREQ },
        >::configure_clocks(&mut flash, &mut rcc, &mut pwr);

        // Initialize the systick interrupt & obtain the token to prove that we did
        Mono::start(ctx.core.SYST, clocks.hclk().to_Hz());
        defmt::info!("\tSysTick");

        let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);
        let mut gpiod = dp.GPIOD.split(&mut rcc.ahb2);

        let led = gpioa
            .pa10
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper)
            .set_speed(stm32l4xx_hal::gpio::Speed::Low);

        // EN_TEMP
        gpiod
            .pd10
            .into_push_pull_output(&mut gpiod.moder, &mut gpiod.otyper)
            .set_speed(stm32l4xx_hal::gpio::Speed::Low)
            .set_high();

        // EN_PRES
        gpiod
            .pd13
            .into_push_pull_output(&mut gpiod.moder, &mut gpiod.otyper)
            .set_speed(stm32l4xx_hal::gpio::Speed::Low)
            .set_high();

        //---------------------------------------------------------------------

        //toggle_led::spawn().unwrap();

        (Shared {}, Local { led })
    }

    //-------------------------------------------------------------------------

    #[task(local = [led])]
    async fn toggle_led(ctx: toggle_led::Context) {
        let led = ctx.local.led;

        loop {
            led.toggle();
            Mono::delay(1.secs()).await;
        }
    }
}
