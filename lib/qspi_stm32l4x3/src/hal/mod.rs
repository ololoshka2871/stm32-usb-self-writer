pub mod enable;
pub mod iqspi;
pub mod qspi;
pub mod qspi_shared_channel;
pub mod rcc;

// нельзя использовать stm32l4xx_hal::sealed - приватная
mod sealed {
    pub trait Sealed {}
}
pub(crate) use sealed::Sealed;

use crate::qspi::{AddressSize, ClockMode, SampleShift};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct QspiConfig {
    /// This field defines the scaler factor for generating CLK based on the AHB clock
    /// (value+1).
    clock_prescaler: u8,
    /// Number of bytes in Flash memory = 2^[FSIZE+1]
    flash_size: u8,
    address_size: AddressSize,
    /// This bit indicates the level that CLK takes between commands Mode 0(low) / mode 3(high)
    clock_mode: ClockMode,
    /// FIFO threshold level (Activates FTF, QUADSPI_SR[2]) 0-15.
    fifo_threshold: u8,
    sample_shift: SampleShift,
    /// CSHT+1 defines the minimum number of CLK cycles which the chip select (nCS) must
    /// remain high between commands issued to the Flash memory.
    chip_select_high_time: u8,
    qpi_mode: bool,
}

impl Default for QspiConfig {
    fn default() -> QspiConfig {
        QspiConfig {
            clock_prescaler: 0,
            flash_size: 22, // 8MB // 26 = 128MB
            address_size: AddressSize::Addr24Bit,
            clock_mode: ClockMode::Mode0,
            fifo_threshold: 1,
            sample_shift: SampleShift::HalfACycle,
            chip_select_high_time: 1,
            qpi_mode: false,
        }
    }
}

impl QspiConfig {
    pub fn clock_prescaler(mut self, clk_pre: u8) -> Self {
        self.clock_prescaler = clk_pre;
        self
    }

    pub fn flash_size(mut self, fl_size: u8) -> Self {
        self.flash_size = fl_size;
        self
    }

    pub fn address_size(mut self, add_size: AddressSize) -> Self {
        self.address_size = add_size;
        self
    }

    pub fn clock_mode(mut self, clk_mode: ClockMode) -> Self {
        self.clock_mode = clk_mode;
        self
    }

    pub fn fifo_threshold(mut self, fifo_thres: u8) -> Self {
        self.fifo_threshold = fifo_thres;
        self
    }

    pub fn sample_shift(mut self, shift: SampleShift) -> Self {
        self.sample_shift = shift;
        self
    }

    pub fn chip_select_high_time(mut self, csht: u8) -> Self {
        self.chip_select_high_time = csht;
        self
    }

    pub fn qpi_mode(mut self, qpi: bool) -> Self {
        self.qpi_mode = qpi;
        self
    }
}
