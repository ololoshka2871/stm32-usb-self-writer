mod high_performance_clocking;
mod recorder_clocking;

pub mod ext_rtc;
pub mod rtc;

pub use high_performance_clocking::{HighPerformanceClockConfigProvider, PllConfigProvider};
pub use recorder_clocking::RecorderClockConfigProvider;

use stm32l4xx_hal::{rcc::PllConfig, time::Hertz};

#[derive(Clone, Copy, Debug, Default)]
pub struct CurrentTime {
    pub year: u32,
    pub month: u32,
    pub day_of_month: u32,
    pub day_of_week: u32,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub milliseconds: u32,
}

impl defmt::Format for CurrentTime {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            self.year,
            self.month,
            self.day_of_month,
            self.hours,
            self.minutes,
            self.seconds,
            self.milliseconds
        );
    }
}

impl Into<u64> for CurrentTime {
    fn into(self) -> u64 {
        let year = self.year as u64;
        let month = self.month as u64;
        let hours = self.hours as u64;
        let minutes = self.minutes as u64;
        let seconds = self.seconds as u64;
        let milliseconds = self.milliseconds as u64;

        // Calendar-accurate conversion to milliseconds since Unix epoch (1970-01-01T00:00:00Z)
        let mut days = 0;
        for y in 1970..year {
            days += if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
                366
            } else {
                365
            };
        }

        let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for m in 0..month {
            days += month_days[m as usize];
            if m == 1 && ((year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)) {
                days += 1; // Leap day
            }
        }

        days * 24 * 60 * 60 * 1000
            + hours * 60 * 60 * 1000
            + minutes * 60 * 1000
            + seconds * 1000
            + milliseconds
    }
}

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

#[derive(Clone, Copy, Debug)]
pub enum I2CRtcError {
    I2cReadError,
    I2cWriteError,
    UnsupportedSetting,
}

pub trait I2CRtcCtrl {
    fn current_time(&mut self) -> Result<CurrentTime, I2CRtcError>;
    fn set_time(&mut self, time: CurrentTime) -> Result<(), I2CRtcError>;

    fn set_tick_period(&mut self, period: Hertz) -> Result<(), I2CRtcError>;

    fn dump_registers(&mut self) -> Result<(), I2CRtcError> {
        // Default implementation does nothing, as not all RTCs may support this
        Ok(())
    }
}

pub struct RtcCalibrationOutputPin(bool);

impl RtcCalibrationOutputPin {
    pub fn new(is_remap: bool) -> Self {
        Self(is_remap)
    }

    pub fn is_remap(&self) -> bool {
        self.0
    }
}

pub trait RtcCalibrationOutput {
    fn enable_calibration_output(
        &mut self,
        pin: impl Into<RtcCalibrationOutputPin>,
        frequency: Hertz,
    ) -> Result<(), ()>;
}
