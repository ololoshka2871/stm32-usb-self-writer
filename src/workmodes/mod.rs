//pub mod high_performance_mode;
//pub mod recorder_mode;

//pub mod common;
//mod my_clock_freeze;

pub mod output_storage;
//pub mod processing;

#[derive(Clone, Copy, Debug, PartialEq, defmt::Format)]
pub enum FChannel {
    Pressure = 0,
    Temperature = 1,
}