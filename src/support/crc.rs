pub trait ZlibCompantCrc32 {
    /// Resets the CRC calculation unit to its initial state.
    fn reset(&mut self);

    /// Feeds data into the CRC calculation unit.
    fn feed(&mut self, data: &[u8]);

    /// Retrieves the current CRC result.
    fn result(&self) -> u32;
}

pub struct STM32L4Crc32(stm32l4xx_hal::crc::Crc);

impl STM32L4Crc32 {
    pub fn new(config: stm32l4xx_hal::crc::Config) -> Self {
        let configured_crc = config
            // теперь результат соответсвует zlib овскому, но !нужно инвертировать!
            // https://stackoverflow.com/a/48883954
            .input_bit_reversal(stm32l4xx_hal::crc::BitReversal::ByByte)
            .output_bit_reversal(true)
            .freeze();

        Self(configured_crc)
    }
}

impl ZlibCompantCrc32 for STM32L4Crc32 {
    fn reset(&mut self) {
        self.0.reset();
    }

    fn feed(&mut self, data: &[u8]) {
        self.0.feed(data);
    }

    // https://docs.rs/stm32l4xx-hal/0.6.0/stm32l4xx_hal/crc/index.html
    fn result(&self) -> u32 {
        !self.0.peek_result()
    }
}
