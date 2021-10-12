use freertos_rust::{Duration, Task, TaskPriority};
use stm32l4xx_hal::rcc::{PllConfig, PllDivider};
use stm32l4xx_hal::{prelude::*, stm32};

use crate::threads;
use crate::workmodes::common::{calc_monitoring_period, enable_dma_clocking};

use super::WorkMode;

// see: src/config/FreeRTOSConfig.h: configMAX_SYSCALL_INTERRUPT_PRIORITY
static IRQ_HIGEST_PRIO: u8 = 80;

/// USB interrupt ptiority
static USB_INTERRUPT_PRIO: u8 = IRQ_HIGEST_PRIO + 1;

pub struct HighPerformanceMode {
    rcc: stm32l4xx_hal::rcc::Rcc,
    flash: stm32l4xx_hal::flash::Parts,
    pwr: stm32l4xx_hal::pwr::Pwr,

    clocks: Option<stm32l4xx_hal::rcc::Clocks>,

    usb: stm32l4xx_hal::stm32::USB,
    gpioa: stm32l4xx_hal::gpio::gpioa::Parts,

    nvic: cortex_m::peripheral::NVIC,
}

impl HighPerformanceMode {
    fn set_interrupt_prio(&mut self, irq: stm32l4xx_hal::stm32l4::stm32l4x2::Interrupt, prio: u8) {
        unsafe {
            self.nvic.set_priority(irq, prio);
        }
    }
}

impl WorkMode<HighPerformanceMode> for HighPerformanceMode {
    fn new(p: cortex_m::Peripherals, dp: stm32l4xx_hal::stm32l4::stm32l4x2::Peripherals) -> Self {
        let mut rcc = dp.RCC.constrain();

        HighPerformanceMode {
            flash: dp.FLASH.constrain(),
            usb: dp.USB,

            gpioa: dp.GPIOA.split(&mut rcc.ahb2),
            pwr: dp.PWR.constrain(&mut rcc.apb1r1),
            clocks: None,

            nvic: p.NVIC,

            rcc: rcc,
        }
    }

    // Работа от внешнего кварца HSE = 12 MHz
    // Установить частоту CPU = 80 MHz (12 / 3 * 40 / 2 == 80)
    // USB работает от PLLSAI1Q = 48 MHz (12 / 3 * 24 / 2 == 48)
    fn configure_clock(&mut self) {
        fn configure_usb48() {
            let _rcc = unsafe { &*stm32::RCC::ptr() };

            // set USB 48Mhz clock src to PLLSAI1Q
            // mast be configured only before PLL enable

            _rcc.cr.modify(|_, w| w.pllsai1on().clear_bit());
            while _rcc.cr.read().pllsai1rdy().bit_is_set() {}

            _rcc.pllsai1cfgr.modify(|_, w| unsafe {
                w.pllsai1n()
                    .bits(24) // * 24
                    .pllsai1q()
                    .bits(0b00) // /2
                    .pllsai1qen()
                    .set_bit() // enable PLLSAI1Q
            });

            _rcc.cr.modify(|_, w| w.pllsai1on().set_bit());
            while _rcc.cr.read().pllsai1rdy().bit_is_set() {}

            // PLLSAI1Q -> CLK48MHz
            unsafe { _rcc.ccipr.modify(|_, w| w.clk48sel().bits(0b01)) };
        }

        fn setut_cfgr(work_cfgr: &mut stm32l4xx_hal::rcc::CFGR) {
            let mut cfgr = unsafe {
                core::mem::MaybeUninit::<stm32l4xx_hal::rcc::CFGR>::zeroed().assume_init()
            };

            core::mem::swap(&mut cfgr, work_cfgr);

            let mut cfgr = cfgr
                .hsi48(true)
                .hse(
                    crate::workmodes::common::HSE_FREQ, // onboard crystall
                    stm32l4xx_hal::rcc::CrystalBypass::Disable,
                    stm32l4xx_hal::rcc::ClockSecuritySystem::Enable,
                )
                .sysclk_with_pll(
                    80.mhz(),                                // CPU clock
                    PllConfig::new(3, 40, PllDivider::Div2), // PLL config
                )
                .pll_source(stm32l4xx_hal::rcc::PllSource::HSE)
                .pclk1(80.mhz())
                .pclk2(80.mhz());

            core::mem::swap(&mut cfgr, work_cfgr);
        }

        setut_cfgr(&mut self.rcc.cfgr);

        let clocks = self.rcc.cfgr.freeze(&mut self.flash.acr, &mut self.pwr);
        configure_usb48();

        enable_dma_clocking();

        self.clocks = Some(clocks);
    }

    fn start_threads(mut self) -> Result<(), freertos_rust::FreeRtosError> {
        use stm32l4xx_hal::stm32l4::stm32l4x2::Interrupt;

        defmt::trace!("Set usb interrupt prio = {}", USB_INTERRUPT_PRIO);
        self.set_interrupt_prio(Interrupt::USB, USB_INTERRUPT_PRIO);

        {
            defmt::trace!("Creating usb thread...");
            let usbperith = threads::usbd::UsbdPeriph {
                usb: self.usb,
                gpioa: self.gpioa,
            };
            Task::new()
                .name("Usbd")
                .stack_size(2048)
                .priority(TaskPriority(2))
                .start(move |_| threads::usbd::usbd(usbperith))?;
        }
        // ---
        {
            defmt::trace!("Creating monitor thread...");
            let monitoring_period =
                calc_monitoring_period(Duration::ms(1000), self.clocks.unwrap().sysclk());
            Task::new()
                .name("Monitord")
                .stack_size(1024)
                .priority(TaskPriority(1))
                .start(move |_| threads::monitor::monitord(monitoring_period))?;
        }
        Ok(())
    }

    fn print_clock_config(&self) {
        super::common::print_clock_config(&self.clocks, "HSI48");
    }
}
