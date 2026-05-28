use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use stm32l4xx_hal::time::Hertz;

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

impl<I2C: Write + Read + 'static> Rx8130ce<I2C> {
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }
}

impl<I2C: Write + Read + WriteRead + 'static> I2CRtcCtrl for Rx8130ce<I2C> {
    fn current_time(&mut self) -> Result<CurrentTime, I2CRtcError> {
        let mut raw = [0_u8; 7];
        self.i2c
            .write_read(RX8130CE_I2C_ADDR, &[REG_SECONDS], &mut raw)
            .map_err(|_| I2CRtcError::I2cReadError)?;

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

        let data: &[u8] = &regs;
        let mut payload = [0_u8; 8];
        payload[0] = REG_SECONDS;
        payload[1..(data.len() + 1)].copy_from_slice(data);
        self.i2c
            .write(RX8130CE_I2C_ADDR, &payload[..(data.len() + 1)])
            .map_err(|_| I2CRtcError::I2cWriteError)
    }

    fn set_tick_period(&mut self, period: Hertz) -> Result<(), I2CRtcError> {
        const EXTANSION_REG: u8 = 0x1C;
        const FSEL0_BIT_POS: u8 = 6;

        #[derive(Clone, Copy)]
        enum TickRate {
            Hz32768 = 0b00,
            Hz1024 = 0b01,
            Hz1 = 0b10,
            Hz0 = 0b11,
        }

        impl TickRate {
            pub fn mask() -> u8 {
                0b11 << FSEL0_BIT_POS
            }

            pub fn to_bits(&self) -> u8 {
                (*self as u8) << FSEL0_BIT_POS
            }
        }

        let data = match period.to_Hz() {
            32_768 => TickRate::Hz32768,
            1_024 => TickRate::Hz1024,
            1 => TickRate::Hz1,
            0 => TickRate::Hz0,
            _ => return Err(I2CRtcError::UnsupportedSetting),
        };

        let mut current = [0_u8; 1];
        self.i2c
            .write_read(RX8130CE_I2C_ADDR, &[EXTANSION_REG], &mut current)
            .map_err(|_| I2CRtcError::I2cWriteError)?;

        let new_value = (current[0] & !TickRate::mask()) | data.to_bits();
        self.i2c
            .write(RX8130CE_I2C_ADDR, &[EXTANSION_REG, new_value])
            .map_err(|_| I2CRtcError::I2cWriteError)
    }

    fn dump_registers(&mut self) -> Result<(), I2CRtcError> {
        #[repr(usize)]
        #[allow(non_camel_case_types, unused)]
        #[derive(Clone, Copy, defmt::Format)]
        enum Register {
            SEC = 0x10,
            MIN = 0x11,
            HOUR = 0x12,
            WEEK = 0x13,
            DAY = 0x14,
            MONTH = 0x15,
            YEAR = 0x16,
            MIN_Alarm = 0x17,
            HOUR_Alarm = 0x18,
            WEEK_Alarm = 0x19,
            Timer_Counter_0 = 0x1A,
            Timer_Counter_1 = 0x1B,
            Extension_Register = 0x1C,
            Flag_Register = 0x1D,
            Control_Register_0 = 0x1E,
            Control_Register_1 = 0x1F,
        }

        impl From<usize> for Register {
            fn from(value: usize) -> Self {
                unsafe { core::mem::transmute(value) }
            }
        }

        const REG_START: u8 = 0x10;
        const REG_COUNT: usize = 0x20 - REG_START as usize;
        let mut regs = [0_u8; REG_COUNT];
        self.i2c
            .write_read(RX8130CE_I2C_ADDR, &[REG_START], &mut regs)
            .map_err(|_| I2CRtcError::I2cReadError)?;

        defmt::info!("RX-8130CE Registers:");
        for (i, reg) in regs.iter().enumerate() {
            defmt::info!(
                "Reg {:02X}:\t0b{:08b}",
                Register::from(i + REG_START as usize),
                reg
            );
        }

        Ok(())
    }
}
