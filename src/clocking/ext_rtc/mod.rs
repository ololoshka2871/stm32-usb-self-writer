use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

use crate::clocking::{CurrentTime, I2CRtcCtrl, I2CRtcError};

mod rv_3028_v7;
mod rx8130ce;

pub(super) fn dec2bcd(value: u8) -> u8 {
    ((value / 10) << 4) | (value % 10)
}

pub(super) fn bcd2dec(value: u8) -> u8 {
    ((value >> 4) * 10) + (value & 0x0F)
}

pub use rv_3028_v7::{RV3028V7_I2C_ADDR, Rv3028v7};
pub use rx8130ce::{RX8130CE_I2C_ADDR, Rx8130ce};

pub enum ExtRtcType<I2C: Write + Read + WriteRead + 'static> {
    Rx8130ce(Rx8130ce<I2C>),
    Rv3028v7(Rv3028v7<I2C>),
}

impl<I2C: Write + Read + WriteRead + 'static> I2CRtcCtrl for ExtRtcType<I2C> {
    fn current_time(&mut self) -> Result<CurrentTime, I2CRtcError> {
        match self {
            ExtRtcType::Rx8130ce(rtc) => rtc.current_time(),
            ExtRtcType::Rv3028v7(rtc) => rtc.current_time(),
        }
    }

    fn set_time(&mut self, time: CurrentTime) -> Result<(), I2CRtcError> {
        match self {
            ExtRtcType::Rx8130ce(rtc) => rtc.set_time(time),
            ExtRtcType::Rv3028v7(rtc) => rtc.set_time(time),
        }
    }

    fn set_alarm_period_ms(&mut self, period_ms: u32) -> Result<(), I2CRtcError> {
        match self {
            ExtRtcType::Rx8130ce(rtc) => rtc.set_alarm_period_ms(period_ms),
            ExtRtcType::Rv3028v7(rtc) => rtc.set_alarm_period_ms(period_ms),
        }
    }
}

impl<I2C: Write + Read + WriteRead + 'static> defmt::Format for ExtRtcType<I2C> {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            ExtRtcType::Rx8130ce(_) => defmt::write!(fmt, "RX-8130CE"),
            ExtRtcType::Rv3028v7(_) => defmt::write!(fmt, "RV-3028-V7"),
        }
    }
}

pub fn try_detect_i2c_rtc<I2C: Write + Read + WriteRead + 'static>(
    mut i2c: I2C,
) -> Result<ExtRtcType<I2C>, I2C> {
    if i2c.write(RX8130CE_I2C_ADDR, &[]).is_ok() {
        Ok(ExtRtcType::Rx8130ce(Rx8130ce::new(i2c)))
    } else if i2c.write(RV3028V7_I2C_ADDR, &[]).is_ok() {
        Ok(ExtRtcType::Rv3028v7(Rv3028v7::new(i2c)))
    } else {
        Err(i2c)
    }
}
