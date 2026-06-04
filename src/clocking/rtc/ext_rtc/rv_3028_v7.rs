use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use stm32l4xx_hal::time::Hertz;

use crate::clocking::rtc::{CurrentTime, I2CRtcCtrl, I2CRtcError};

use super::{bcd2dec, dec2bcd};

pub const RV3028V7_I2C_ADDR: u8 = 0x52;
const REG_SECONDS: u8 = 0x00;

pub struct Rv3028v7<I2C> {
    i2c: I2C,
}

impl<I2C: Write + Read + WriteRead + 'static> Rv3028v7<I2C> {
    pub fn new(i2c: I2C) -> Self {
        let mut res = Self { i2c };
        res.configure_bsm().unwrap();
        res
    }

    fn configure_bsm(&mut self) -> Result<(), I2CRtcError> {
        const EEPROM_BACKUP_REGISTER_REG: u8 = 0x37;
        const BSM_MODE: u8 = 0b11 << 2;
        const BSM_MODE_MASK: u8 = 0b11 << 2;
        const TCE_MASK: u8 = 0b1 << 6;

        let mut current = [0_u8; 1];
        self.i2c
            .write_read(
                RV3028V7_I2C_ADDR,
                &[EEPROM_BACKUP_REGISTER_REG],
                &mut current,
            )
            .map_err(|_| I2CRtcError::I2cReadError)?;

        let new_value = (current[0] & !BSM_MODE_MASK & !TCE_MASK) | BSM_MODE;
        self.i2c
            .write(RV3028V7_I2C_ADDR, &[EEPROM_BACKUP_REGISTER_REG, new_value])
            .map_err(|_| I2CRtcError::I2cWriteError)
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
        const EEPROM_CLKOUT_REG: u8 = 0x35;

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

        let mut current = [0_u8; 1];
        self.i2c
            .write_read(RV3028V7_I2C_ADDR, &[EEPROM_CLKOUT_REG], &mut current)
            .map_err(|_| I2CRtcError::I2cWriteError)?;

        let new_value = (current[0] & !TickRate::mask()) | data.to_bits() | (1 << 7); // Enable CLKOUT output
        self.i2c
            .write(RV3028V7_I2C_ADDR, &[EEPROM_CLKOUT_REG, new_value])
            .map_err(|_| I2CRtcError::I2cWriteError)
    }

    fn dump_registers(&mut self) -> Result<(), I2CRtcError> {
        const START: u8 = 0x35;
        let mut raw = [0_u8; 3];

        self.i2c
            .write_read(RV3028V7_I2C_ADDR, &[START], &mut raw)
            .map_err(|_| I2CRtcError::I2cReadError)?;

        defmt::info!("RV-3028-V7 RTC registers:");
        for (i, byte) in raw.iter().enumerate() {
            defmt::info!("Reg 0x{:02X}: 0b{:08b}", i + START as usize, byte);
        }

        Ok(())
    }
}
