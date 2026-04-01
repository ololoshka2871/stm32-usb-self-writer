use super::{DateTime, Rtc, bcd2dec, dec2bcd};

pub const RX8130_I2C_ADDR: u8 = 0x32;

pub struct Rx8130<I> {
    i2c: I,
}

impl<I, E> Rx8130<I>
where
    I: embedded_hal::blocking::i2c::WriteRead<Error = E>
        + embedded_hal::blocking::i2c::Write<Error = E>,
{
    pub fn new(i2c: I) -> Self {
        Rx8130 { i2c }
    }
}

impl<I, E> Rtc for Rx8130<I>
where
    I: embedded_hal::blocking::i2c::WriteRead<Error = E>
        + embedded_hal::blocking::i2c::Write<Error = E>,
{
    fn set_time(&mut self, dt: DateTime) -> Result<(), ()> {
        // RX8130: seconds @ 0x00, minutes @ 0x01, hours @ 0x02, week @ 0x03, day @ 0x04, month @ 0x05, year @ 0x06
        let buf = [
            0x00, // start register
            dec2bcd(dt.second),
            dec2bcd(dt.minute),
            dec2bcd(dt.hour),
            0, // week (not used)
            dec2bcd(dt.day),
            dec2bcd(dt.month),
            dec2bcd((dt.year % 100) as u8),
        ];
        self.i2c.write(RX8130_I2C_ADDR, &buf).map_err(|_| ())
    }

    fn get_time(&mut self) -> Result<DateTime, ()> {
        // Read 7 bytes from 0x00 (seconds, minutes, hours, week, day, month, year)
        let mut regs = [0u8; 7];
        self.i2c.write_read(RX8130_I2C_ADDR, &[0x00], &mut regs).map_err(|_| ())?;
        Ok(DateTime {
            second: bcd2dec(regs[0] & 0x7F),
            minute: bcd2dec(regs[1] & 0x7F),
            hour: bcd2dec(regs[2] & 0x3F),
            day: bcd2dec(regs[4] & 0x3F),
            month: bcd2dec(regs[5] & 0x1F),
            year: 2000 + bcd2dec(regs[6]) as u16,
            ms: 0,
        })
    }

    fn enable_1hz_exti(&mut self) -> Result<(), ()> {
        // RX8130: Control register 0x0F, OUT bit (bit 3), FOUT register 0x17
        // 1. Set FOUT to 1Hz (0x10)
        self.i2c.write(RX8130_I2C_ADDR, &[0x17, 0x10]).map_err(|_| ())?;
        // 2. Enable OUT bit in control register
        let mut ctrl = [0u8; 1];
        self.i2c.write_read(RX8130_I2C_ADDR, &[0x0F], &mut ctrl).map_err(|_| ())?;
        let new_ctrl = ctrl[0] | (1 << 3);
        self.i2c.write(RX8130_I2C_ADDR, &[0x0F, new_ctrl]).map_err(|_| ())?;
        Ok(())
    }
}
