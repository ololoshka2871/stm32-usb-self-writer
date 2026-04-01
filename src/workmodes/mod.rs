#[allow(unused_imports)]
use stm32l4xx_hal::gpio::{
    Alternate, Output, PushPull, PA2, PA3, PA6, PA7, PB0, PB1, PD0, PD3, PE12,
};

use alloc::sync::Arc;
use freertos_rust::{FreeRtosError, Mutex};

pub mod high_performance_mode;
pub mod recorder_mode;

pub(crate) mod common;
//mod my_clock_freeze;

pub mod output_storage;
pub mod processing;

#[cfg(feature = "maket")]
type D0Pin = PE12<Alternate<PushPull, 10>>;

#[cfg(not(feature = "maket"))]
type D0Pin = PB1<Alternate<PushPull, 10>>;

pub type Flash = qspi_stm32lx3::qspi::Qspi<(
    PA3<Alternate<PushPull, 10>>,
    PA2<Alternate<PushPull, 10>>,
    D0Pin,
    PB0<Alternate<PushPull, 10>>,
    PA7<Alternate<PushPull, 10>>,
    PA6<Alternate<PushPull, 10>>,
)>;

#[cfg(feature = "maket")]
pub type TP1 = PD3<Output<PushPull>>;
#[cfg(not(feature = "maket"))]
pub type TP1 = PD0<Output<PushPull>>;

pub trait WorkMode<T> {
    fn new(p: cortex_m::Peripherals, dp: stm32l4xx_hal::device::Peripherals) -> T;
    fn ini_static(&mut self);
    fn configure_clock(&mut self);
    fn start_threads(self) -> Result<(), FreeRtosError>;
    fn print_clock_config(&self);
    fn flash(&mut self) -> Arc<Mutex<stm32l4xx_hal::flash::Parts>>;
    fn crc(&mut self) -> Arc<Mutex<stm32l4xx_hal::crc::Crc>>;
}

fn configure_crc_module(config: stm32l4xx_hal::crc::Config) -> stm32l4xx_hal::crc::Crc {
    config
        // теперь результат соответсвует zlib овскому, но !нужно инвертировать!
        // https://stackoverflow.com/a/48883954
        .input_bit_reversal(stm32l4xx_hal::crc::BitReversal::ByByte)
        .output_bit_reversal(true)
        .freeze()
}
