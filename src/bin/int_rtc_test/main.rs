#![no_std]
#![no_main]

extern crate alloc;

use defmt_rtt as _; // global logger
use panic_abort as _;

use stm32l4xx_hal::prelude::*;

use rtic::app;
use rtic_monotonics::Monotonic;

use stm32_usb_self_writer::{
    clocking::{RtcCalibrationOutput, rtc::RtcService},
    config,
};

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

#[app(device = stm32l4xx_hal::pac, peripherals = true, dispatchers = [RCC, LCD, TAMP_STAMP, SWPMI1])]
mod app {
    use super::*;

    #[shared]
    struct Shared {
        rtc: RtcService,
    }

    #[local]
    struct Local {}

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

        let mut flash = dp.FLASH.constrain();
        let mut rcc = dp.RCC.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);

        let clocks = rcc.cfgr.freeze(&mut flash.acr, &mut pwr);

        unsafe {
            #[allow(static_mut_refs)]
            umm_malloc::init_heap(HEAP.as_mut_ptr() as usize, config::HEAP_SIZE)
        };

        defmt::info!("\tHeap");

        // Initialize the systick interrupt & obtain the token to prove that we did
        Mono::start(ctx.core.SYST, clocks.hclk().to_Hz());
        defmt::info!("\tSysTick");

        let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);
        let mut gpioc = dp.GPIOC.split(&mut rcc.ahb2);

        let (mut rtc, cs) = stm32_usb_self_writer::clocking::rtc::RtcService::init(
            dp.RTC,
            &mut dp.EXTI,
            &mut rcc.apb1r1,
            &mut rcc.bdcr,
            &mut pwr.cr1,
        );
        rtc.set_alarm_period_ms(1_000);
        rtc.enable_calibration_output(
            /*gpiob.pb2.into_alternate_push_pull(
                &mut gpiob.moder,
                &mut gpiob.otyper,
                &mut gpiob.afrl,
            ),*/
            gpioc.pc13,
            stm32l4xx_hal::time::Hertz::Hz(512),
        )
        .unwrap();

        defmt::info!("\tRTC: {}", defmt::Debug2Format(&cs));

        //---------------------------------------------------------------------

        (Shared { rtc }, Local {})
    }

    //-------------------------------------------------------------------------

    #[task(binds = USART3, shared = [rtc])]
    fn uart3(ctx: uart3::Context) {
        /* TODO */
    }

    // Приоритет строго равен sync_freqmeter*, иначе Deadlock на мьютексе rtc_sync
    #[task(binds = RTC_WKUP, shared = [rtc], priority = 4)]
    fn rtc_alarm(ctx: rtc_alarm::Context) {
        let mut rtc = ctx.shared.rtc;

        rtc.lock(|rtc| rtc.handle_alarm_interrupt());

        defmt::debug!("Alarm interrupt");
    }
}
