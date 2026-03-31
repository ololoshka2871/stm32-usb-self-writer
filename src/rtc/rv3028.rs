use super::{DateTime, Rtc, bcd2dec, dec2bcd};

pub const RV3028_I2C_ADDR: u8 = 0x52;

pub struct Rv3028<I> {
    i2c: I,
}

impl<I, E> Rv3028<I>
where
    I: embedded_hal::blocking::i2c::WriteRead<Error = E>
        + embedded_hal::blocking::i2c::Write<Error = E>,
{
    pub fn new(i2c: I) -> Self {
        Rv3028 { i2c }
    }
}

impl<I, E> Rtc for Rv3028<I>
where
    I: embedded_hal::blocking::i2c::WriteRead<Error = E>
        + embedded_hal::blocking::i2c::Write<Error = E>,
{
    fn set_time(&mut self, dt: DateTime) -> Result<(), ()> {
        // RV-3028: seconds @ 0x00, minutes @ 0x01, hours @ 0x02, date @ 0x03, month @ 0x04, year @ 0x05
        let buf = [
            0x00, // start register
            dec2bcd(dt.second),
            dec2bcd(dt.minute),
            dec2bcd(dt.hour),
            dec2bcd(dt.day),
            dec2bcd(dt.month),
            dec2bcd((dt.year % 100) as u8),
        ];
        // Write time registers (skip week day at 0x06)
        self.i2c.write(RV3028_I2C_ADDR, &buf).map_err(|_| ())
    }

    fn get_time(&mut self) -> Result<DateTime, ()> {
        // Read 7 bytes from 0x00 (seconds, minutes, hours, date, month, year, weekday)
        let mut regs = [0u8; 7];
        self.i2c
            .write_read(RV3028_I2C_ADDR, &[0x00], &mut regs)
            .map_err(|_| ())?;
        Ok(DateTime {
            second: bcd2dec(regs[0] & 0x7F),
            minute: bcd2dec(regs[1] & 0x7F),
            hour: bcd2dec(regs[2] & 0x3F),
            day: bcd2dec(regs[3] & 0x3F),
            month: bcd2dec(regs[4] & 0x1F),
            year: 2000 + bcd2dec(regs[5]) as u16,
        })
    }

    fn enable_1hz_exti(&mut self) -> Result<(), ()> {
        // RV-3028: Control 2 (0x37), set bit 3 (CLKOUT enable)
        // CLKOUT freq: CLKOUT register (0x35), set to 0b0000_0001 for 1Hz
        // 1. Set 1Hz in CLKOUT
        self.i2c
            .write(RV3028_I2C_ADDR, &[0x35, 0x01])
            .map_err(|_| ())?;
        // 2. Enable CLKOUT in Control2
        // Read-modify-write Control2
        let mut ctrl2 = [0u8; 1];
        self.i2c
            .write_read(RV3028_I2C_ADDR, &[0x37], &mut ctrl2)
            .map_err(|_| ())?;
        let new_ctrl2 = ctrl2[0] | (1 << 3);
        self.i2c
            .write(RV3028_I2C_ADDR, &[0x37, new_ctrl2])
            .map_err(|_| ())?;
        Ok(())
    }
}
