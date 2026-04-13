use core::marker::PhantomData;

use stm32l4xx_hal::{
    rcc::{PllConfig, PllDivider},
    stm32,
    time::Hertz,
};

pub fn to_pll_devider(v: u32) -> PllDivider {
    match v {
        2 => PllDivider::Div2,
        4 => PllDivider::Div4,
        6 => PllDivider::Div6,
        8 => PllDivider::Div8,
        _ => panic!(),
    }
}

pub fn to_sai_divider(v: u8) -> u8 {
    match v {
        2 => 0b00,
        4 => 0b01,
        8 => 0b10,
        16 => 0b11,
        _ => panic!(),
    }
}

pub trait PllConfigProvider {
    const PD: u32;
    const M: u32;
    const AD: u32;

    const SAI_MUL: u32;
    const SAI_DIV_CODE: u32;
}

pub struct HighPerformanceClockConfigProvider<
    PLL: PllConfigProvider,
    const XTAL_FREQ: u32,
    const CPU_FREQ: u32,
    const APB1_DEVIDER: u32 = 1,
    const APB2_DEVIDER: u32 = 8,
>(PhantomData<PLL>);

impl<
        PLL: PllConfigProvider,
        const XTAL_FREQ: u32,
        const CPU_FREQ: u32,
        const APB1_DEVIDER: u32,
        const APB2_DEVIDER: u32,
    > super::ClockConfigProvider
    for HighPerformanceClockConfigProvider<PLL, XTAL_FREQ, CPU_FREQ, APB1_DEVIDER, APB2_DEVIDER>
{
    fn core_frequency() -> Hertz {
        let f = XTAL_FREQ * PLL::M / (PLL::PD * PLL::AD);
        Hertz(f)
    }

    fn apb1_frequency() -> Hertz {
        Hertz(Self::core_frequency().0 / APB1_DEVIDER)
    }

    fn apb2_frequency() -> Hertz {
        Hertz(Self::core_frequency().0 / APB2_DEVIDER)
    }

    // stm32_cube: if APB devider > 1, timers freq APB*2
    fn master_counter_frequency() -> Hertz {
        if APB1_DEVIDER > 1 {
            Hertz(Self::core_frequency().0 / APB1_DEVIDER * 2)
        } else {
            Hertz(Self::core_frequency().0 / APB1_DEVIDER)
        }
    }

    fn pll_config() -> PllConfig {
        PllConfig::new(PLL::PD as u8, PLL::M as u8, to_pll_devider(PLL::AD))
    }

    fn xtal2master_freq_multiplier() -> f64 {
        if APB1_DEVIDER > 1 {
            Self::core_frequency().0 as f64 / APB1_DEVIDER as f64 * 2.0
        } else {
            Self::core_frequency().0 as f64
        }
    }

    fn configure_clocks(
        flash: &mut stm32l4xx_hal::flash::Parts,
        rcc: &mut stm32l4xx_hal::rcc::Rcc,
        pwr: &mut stm32l4xx_hal::pwr::Pwr,
    ) -> stm32l4xx_hal::rcc::Clocks {
        fn configure_usb48(sai_mul: u8, sai_div: u8) {
            // set USB 48Mhz clock src to PLLSAI1Q
            // mast be configured only before PLL enable
            let rcc = unsafe { &*stm32::RCC::ptr() };

            rcc.cr.modify(|_, w| w.pllsai1on().clear_bit());
            while rcc.cr.read().pllsai1rdy().bit_is_set() {}

            rcc.pllsai1cfgr.modify(|_, w| unsafe {
                w.pllsai1n()
                    .bits(sai_mul)
                    .pllsai1q()
                    .bits(to_sai_divider(sai_div))
                    .pllsai1qen()
                    .set_bit() // enable PLLSAI1Q
            });

            rcc.cr.modify(|_, w| w.pllsai1on().set_bit());
            while rcc.cr.read().pllsai1rdy().bit_is_set() {}

            // PLLSAI1Q -> CLK48MHz
            unsafe { rcc.ccipr.modify(|_, w| w.clk48sel().bits(0b01)) };
        }

        {
            let work_cfgr: &mut stm32l4xx_hal::rcc::CFGR = &mut rcc.cfgr;
            let mut cfgr = unsafe {
                core::mem::MaybeUninit::<stm32l4xx_hal::rcc::CFGR>::zeroed().assume_init()
            };

            core::mem::swap(&mut cfgr, work_cfgr);

            let mut cfgr = cfgr
                .hsi48(false)
                .hse(
                    Hertz(XTAL_FREQ), // onboard crystall
                    stm32l4xx_hal::rcc::CrystalBypass::Disable,
                    stm32l4xx_hal::rcc::ClockSecuritySystem::Enable,
                )
                .sysclk_with_pll(
                    Self::core_frequency(),
                    PllConfig::new(PLL::PD as u8, PLL::M as u8, to_pll_devider(PLL::AD)),
                )
                .pll_source(stm32l4xx_hal::rcc::PllSource::HSE)
                .pclk1(Self::apb1_frequency())
                .pclk2(Self::apb2_frequency());

            core::mem::swap(&mut cfgr, work_cfgr);
        };

        let clocks = rcc.cfgr.freeze(&mut flash.acr, pwr);

        configure_usb48(PLL::SAI_MUL as u8, PLL::SAI_DIV_CODE as u8);

        clocks
    }
}
