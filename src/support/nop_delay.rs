use stm32l4xx_hal::time::Hertz;

pub struct NOPDelay {
    pub sys_clk: Hertz,
}

impl embedded_hal::blocking::delay::DelayUs<u32> for NOPDelay {
    fn delay_us(&mut self, us: u32) {
        cortex_m::asm::delay(self.sys_clk.0 / 1_000_000 * us);
    }
}

impl embedded_hal::blocking::delay::DelayMs<u8> for NOPDelay {
    fn delay_ms(&mut self, ms: u8) {
        cortex_m::asm::delay(self.sys_clk.0 / 1_000 * (ms as u32));
    }
}
