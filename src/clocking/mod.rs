mod high_performance_clocking;
mod recorder_clocking;

pub mod rtc;

pub use high_performance_clocking::{HighPerformanceClockConfigProvider, PllConfigProvider};
pub use recorder_clocking::RecorderClockConfigProvider;

use stm32l4xx_hal::{rcc::PllConfig, time::Hertz};

pub trait ClockConfigProvider {
    fn core_frequency() -> Hertz;
    fn apb1_frequency() -> Hertz;
    fn apb2_frequency() -> Hertz;
    fn master_counter_frequency() -> Hertz;
    fn pll_config() -> PllConfig;
    fn xtal2master_freq_multiplier() -> f64;

    fn configure_clocks(
        flash: &mut stm32l4xx_hal::flash::Parts,
        rcc: &mut stm32l4xx_hal::rcc::Rcc,
        pwr: &mut stm32l4xx_hal::pwr::Pwr,
    ) -> stm32l4xx_hal::rcc::Clocks;
}
