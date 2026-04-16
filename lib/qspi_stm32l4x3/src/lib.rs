#![no_std]

pub mod stm32l4x3;

mod hal;
pub use hal::{iqspi, qspi, qspi_shared_channel, QspiConfig};
