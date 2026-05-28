use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use stm32l4xx_hal::time::Hertz;

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

    fn set_tick_period(&mut self, period: Hertz) -> Result<(), I2CRtcError> {
        const EEPROM_CLKOUT_REG: u8 = 0x1C;

        #[derive(Clone, Copy)]
        enum TickRate {
            Hz32768 = 0b000,
            Hz8192 = 0b001,
            Hz1024 = 0b010,
            Hz64 = 0b011,
            Hz32 = 0b100,
            Hz1 = 0b101,
            Hz0 = 0b111,
        }

        impl TickRate {
            pub fn mask() -> u8 {
                0b111 << 0
            }

            pub fn to_bits(&self) -> u8 {
                (*self as u8) << 0
            }
        }

        let data = match period.to_Hz() {
            32_768 => TickRate::Hz32768,
            8_192 => TickRate::Hz8192,
            1_024 => TickRate::Hz1024,
            64 => TickRate::Hz64,
            32 => TickRate::Hz32,
            1 => TickRate::Hz1,
            0 => TickRate::Hz0,
            _ => return Err(I2CRtcError::UnsupportedSetting),
        };

        self.i2c
            .write(RV3028V7_I2C_ADDR, &[EEPROM_CLKOUT_REG])
            .map_err(|_| I2CRtcError::I2cWriteError)?;

        let mut current = [0_u8; 1];
        self.i2c
            .read(RV3028V7_I2C_ADDR, &mut current)
            .map_err(|_| I2CRtcError::I2cReadError)?;

        let new_value = (current[0] & !TickRate::mask()) | data.to_bits() | (1 << 7); // Enable CLKOUT output
        self.i2c
            .write(RV3028V7_I2C_ADDR, &[EEPROM_CLKOUT_REG, new_value])
            .map_err(|_| I2CRtcError::I2cWriteError)
    }
}
