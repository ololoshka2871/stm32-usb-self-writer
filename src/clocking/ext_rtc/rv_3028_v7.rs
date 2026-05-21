use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

use crate::clocking::{CurrentTime, I2CRtcCtrl, I2CRtcError};

use super::{bcd2dec, dec2bcd};

pub const RV3028V7_I2C_ADDR: u8 = 0x52;
const REG_SECONDS: u8 = 0x00;

pub struct Rv3028v7<I2C> {
    i2c: I2C,
}

impl<I2C> Rv3028v7<I2C> {
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }
}

impl<I2C: Write + Read + WriteRead + 'static> I2CRtcCtrl for Rv3028v7<I2C> {
    fn current_time(&mut self) -> Result<CurrentTime, I2CRtcError> {
        let mut raw = [0_u8; 7];
        {
            let this = &mut *self;
            let data: &mut [u8] = &mut raw;
            this.i2c
                    .write_read(RV3028V7_I2C_ADDR, &[REG_SECONDS], data)
                    .map_err(|_| I2CRtcError::I2cReadError)
        }?;

        Ok(CurrentTime {
            year: 2000 + bcd2dec(raw[6] & 0x7F) as u32,
            month: bcd2dec(raw[5] & 0x1F) as u32,
            day_of_month: bcd2dec(raw[4] & 0x3F) as u32,
            day_of_week: (raw[3] & 0x07) as u32,
            hours: bcd2dec(raw[2] & 0x3F) as u32,
            minutes: bcd2dec(raw[1] & 0x7F) as u32,
            seconds: bcd2dec(raw[0] & 0x7F) as u32,
            milliseconds: 0,
        })
    }

    fn set_time(&mut self, time: CurrentTime) -> Result<(), I2CRtcError> {
        let regs = [
            dec2bcd(time.seconds as u8),
            dec2bcd(time.minutes as u8),
            dec2bcd(time.hours as u8),
            (time.day_of_week as u8) & 0x07,
            dec2bcd(time.day_of_month as u8),
            dec2bcd(time.month as u8),
            dec2bcd((time.year % 100) as u8),
        ];

        {
            let this = &mut *self;
            let data: &[u8] = &regs;
            let mut payload = [0_u8; 8];
            payload[0] = REG_SECONDS;
            payload[1..(data.len() + 1)].copy_from_slice(data);
            this.i2c
                    .write(RV3028V7_I2C_ADDR, &payload[..(data.len() + 1)])
                    .map_err(|_| I2CRtcError::I2cWriteError)
        }
    }

    fn set_alarm_period_ms(&mut self, _period_ms: u32) -> Result<(), I2CRtcError> {
        panic!("RV-3028-V7 periodic alarm is not implemented yet (architecture pending)");
    }
}
