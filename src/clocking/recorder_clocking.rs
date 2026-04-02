use stm32l4xx_hal::{rcc::PllConfig, stm32, time::Hertz};

pub struct RecorderClockConfigProvider<
    const XTAL_FREQ: u32,
    const CPU_FREQ: u32,
    const APB1_DEVIDER: u32 = 1,
    const APB2_DEVIDER: u32 = 1,
>;

impl<
        const XTAL_FREQ: u32,
        const CPU_FREQ: u32,
        const APB1_DEVIDER: u32,
        const APB2_DEVIDER: u32,
    > super::ClockConfigProvider
    for RecorderClockConfigProvider<XTAL_FREQ, CPU_FREQ, APB1_DEVIDER, APB2_DEVIDER>
{
    fn core_frequency() -> Hertz {
        Hertz(CPU_FREQ)
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
            Hertz(Self::apb1_frequency().0 * 2)
        } else {
            Self::apb1_frequency()
        }
    }

    fn pll_config() -> PllConfig {
        unreachable!()
    }

    fn xtal2master_freq_multiplier() -> f64 {
        if APB1_DEVIDER > 1 {
            2.0 / (XTAL_FREQ as f64 / CPU_FREQ as f64)
        } else {
            1.0 / (XTAL_FREQ as f64 / CPU_FREQ as f64)
        }
    }

    fn configure_clocks(
        flash: &mut stm32l4xx_hal::flash::Parts,
        _rcc: &mut stm32l4xx_hal::rcc::Rcc,
        pwr: &mut stm32l4xx_hal::pwr::Pwr,
    ) -> stm32l4xx_hal::rcc::Clocks {
        let clocks = MyCFGR::new()
            .hse(
                Hertz(XTAL_FREQ),
                stm32l4xx_hal::rcc::CrystalBypass::Disable,
                stm32l4xx_hal::rcc::ClockSecuritySystem::Enable,
            )
            .sysclk(Hertz(CPU_FREQ))
            .hclk(Self::core_frequency())
            .pclk1(Self::apb1_frequency())
            .pclk2(Self::apb2_frequency())
            .freeze(&mut flash.acr, pwr);

        // low power run (F <= 2MHz) (на 12 MHz выйгрыш около 200мкА)
        unsafe {
            (*stm32l4xx_hal::device::PWR::ptr())
                .cr1
                .modify(|_, w| w.lpr().set_bit())
        };

        clocks
    }
}

#[derive(Debug, PartialEq)]
/// HSE Configuration
struct HseConfig {
    /// Clock speed of HSE
    speed: u32,
    /// If the clock driving circuitry is bypassed i.e. using an oscillator, not a crystal or
    /// resonator
    bypass: stm32l4xx_hal::rcc::CrystalBypass,
    /// Clock Security System enable/disable
    css: stm32l4xx_hal::rcc::ClockSecuritySystem,
}

struct MyCFGR {
    hse: HseConfig,
    hclk: Option<u32>,
    pclk1: Option<u32>,
    pclk2: Option<u32>,
    sysclk: u32,
}

impl MyCFGR {
    fn new() -> Self {
        Self {
            hse: HseConfig {
                speed: 0,
                bypass: stm32l4xx_hal::rcc::CrystalBypass::Disable,
                css: stm32l4xx_hal::rcc::ClockSecuritySystem::Enable,
            },
            hclk: None,
            pclk1: None,
            pclk2: None,
            sysclk: 0,
        }
    }

    /// Add an HSE to the system
    pub fn hse<F>(
        mut self,
        freq: F,
        bypass: stm32l4xx_hal::rcc::CrystalBypass,
        css: stm32l4xx_hal::rcc::ClockSecuritySystem,
    ) -> Self
    where
        F: Into<Hertz>,
    {
        self.hse = HseConfig {
            speed: freq.into().0,
            bypass,
            css,
        };

        self
    }

    /// Sets a frequency for the AHB bus
    pub fn hclk<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.hclk = Some(freq.into().0);
        self
    }

    /// Sets the system (core) frequency
    pub fn sysclk<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.sysclk = freq.into().0;
        self
    }

    /// Sets a frequency for the APB1 bus
    pub fn pclk1<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.pclk1 = Some(freq.into().0);
        self
    }

    /// Sets a frequency for the APB2 bus
    pub fn pclk2<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.pclk2 = Some(freq.into().0);
        self
    }

    fn freeze(
        &self,
        _acr: &mut stm32l4xx_hal::flash::ACR,
        _pwr: &mut stm32l4xx_hal::pwr::Pwr,
    ) -> stm32l4xx_hal::rcc::Clocks {
        // Поскольку поля stm32l4xx_hal::rcc::Clocks приватные, делает точно такую же
        // структуру, заполняем её и трансмутируем тип core::mem::transmute()
        #[derive(Clone, Copy, Debug)]
        #[allow(dead_code)]
        struct Clocks {
            hclk: Hertz,
            hsi48: bool,
            msi: Option<stm32l4xx_hal::rcc::MsiFreq>,
            lsi: bool,
            lse: bool,
            pclk1: Hertz,
            pclk2: Hertz,
            ppre1: u8,
            ppre2: u8,
            sysclk: Hertz,
            pll_source: Option<stm32l4xx_hal::rcc::PllSource>,
        }

        let rcc = unsafe { &*stm32::RCC::ptr() };
        //
        // 1. Setup clocks
        //

        // If HSE is available, set it up

        rcc.cr.write(|w| {
            w.hseon().set_bit();

            if self.hse.bypass == stm32l4xx_hal::rcc::CrystalBypass::Enable {
                w.hsebyp().set_bit();
            }

            w
        });

        while rcc.cr.read().hserdy().bit_is_clear() {}

        // Setup CSS
        if self.hse.css == stm32l4xx_hal::rcc::ClockSecuritySystem::Enable {
            // Enable CSS
            rcc.cr.modify(|_, w| w.csson().set_bit());
        }

        assert!(self.sysclk <= 80_000_000);

        let (hpre_bits, hpre_div) = self
            .hclk
            .map(|hclk| match self.sysclk / hclk {
                // From p 194 in RM0394
                0 => unreachable!(),
                1 => (0b0000, 1),
                2 => (0b1000, 2),
                3..=5 => (0b1001, 4),
                6..=11 => (0b1010, 8),
                12..=39 => (0b1011, 16),
                40..=95 => (0b1100, 64),
                96..=191 => (0b1101, 128),
                192..=383 => (0b1110, 256),
                _ => (0b1111, 512),
            })
            .unwrap_or((0b0000, 1));

        let hclk = self.sysclk / hpre_div;

        assert!(hclk <= self.sysclk);

        let (ppre1_bits, ppre1) = self
            .pclk1
            .map(|pclk1| match hclk / pclk1 {
                // From p 194 in RM0394
                0 => unreachable!(),
                1 => (0b000, 1),
                2 => (0b100, 2),
                3..=5 => (0b101, 4),
                6..=11 => (0b110, 8),
                _ => (0b111, 16),
            })
            .unwrap_or((0b000, 1));

        let pclk1 = hclk / ppre1 as u32;

        assert!(pclk1 <= self.sysclk);

        let (ppre2_bits, ppre2) = self
            .pclk2
            .map(|pclk2| match hclk / pclk2 {
                // From p 194 in RM0394
                0 => unreachable!(),
                1 => (0b000, 1),
                2 => (0b100, 2),
                3..=5 => (0b101, 4),
                6..=11 => (0b110, 8),
                _ => (0b111, 16),
            })
            .unwrap_or((0b000, 1));

        let pclk2 = hclk / ppre2 as u32;

        assert!(pclk2 <= self.sysclk);

        // adjust flash wait states
        unsafe {
            (*stm32::FLASH::ptr()).acr.write(|w| {
                w.latency().bits(if hclk <= 16_000_000 {
                    0b000
                } else if hclk <= 32_000_000 {
                    0b001
                } else if hclk <= 48_000_000 {
                    0b010
                } else if hclk <= 64_000_000 {
                    0b011
                } else {
                    0b100
                })
            })
        }

        let sysclk_src_bits = 0b10; // HSE

        // HSE: HSE selected as system clock
        rcc.cfgr.write(|w| unsafe {
            w.ppre2()
                .bits(ppre2_bits)
                .ppre1()
                .bits(ppre1_bits)
                .hpre()
                .bits(hpre_bits)
                .sw()
                .bits(sysclk_src_bits)
        });

        while rcc.cfgr.read().sws().bits() != sysclk_src_bits {}

        //
        // 3. Shutdown unused clocks that have auto-started
        //

        // MSI always starts on reset
        {
            rcc.cr
                .modify(|_, w| w.msion().clear_bit().msipllen().clear_bit())
        }

        //
        // 4. Clock setup done!
        //

        unsafe {
            core::mem::transmute(Clocks {
                hclk: Hertz(hclk),
                lsi: false,
                lse: false,
                msi: None,
                hsi48: false,
                pclk1: Hertz(pclk1),
                pclk2: Hertz(pclk2),
                ppre1,
                ppre2,
                sysclk: Hertz(self.sysclk),
                pll_source: None,
            })
        }
    }
}
