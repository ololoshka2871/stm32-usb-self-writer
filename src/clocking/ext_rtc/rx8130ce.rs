use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

use crate::clocking::{CurrentTime, I2CRtcCtrl, I2CRtcError};

use super::{bcd2dec, dec2bcd};

pub const RX8130CE_I2C_ADDR: u8 = 0x32;
const REG_SECONDS: u8 = 0x10;

fn weekday_mask_to_number(mask: u8) -> u8 {
    if mask == 0 {
        0
    } else {
        mask.trailing_zeros() as u8 + 1
    }
}

fn weekday_number_to_mask(day: u8) -> u8 {
    if day == 0 {
        0
    } else {
        1_u8 << ((day - 1) & 0x07)
    }
}

pub struct Rx8130ce<I2C> {
    i2c: I2C,
}

impl<I2C> Rx8130ce<I2C> {
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }
}

impl<I2C: Write + Read + WriteRead + 'static> I2CRtcCtrl for Rx8130ce<I2C> {
    fn current_time(&mut self) -> Result<CurrentTime, I2CRtcError> {
        let mut raw = [0_u8; 7];
        {
            let this = &mut *self;
            let data: &mut [u8] = &mut raw;
            this.i2c
                .write_read(RX8130CE_I2C_ADDR, &[REG_SECONDS], data)
                .map_err(|_| I2CRtcError::I2cReadError)
        }?;

        Ok(CurrentTime {
            year: 2000 + bcd2dec(raw[6] & 0x7F) as u32,
            month: bcd2dec(raw[5] & 0x1F) as u32,
            day_of_month: bcd2dec(raw[4] & 0x3F) as u32,
            day_of_week: weekday_mask_to_number(raw[3] & 0x7F) as u32,
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
            weekday_number_to_mask(time.day_of_week as u8),
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
                .write(RX8130CE_I2C_ADDR, &payload[..(data.len() + 1)])
                .map_err(|_| I2CRtcError::I2cWriteError)
        }
    }

    fn set_alarm_period_ms(&mut self, _period_ms: u32) -> Result<(), I2CRtcError> {
        panic!("RX-8130CE periodic alarm is not implemented yet (architecture pending)");
    }
}
