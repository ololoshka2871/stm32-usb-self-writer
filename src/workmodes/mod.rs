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

impl FChannel {
    pub fn iter(m: u32, n: u32) -> FChannelIter {
        FChannelIter { m, n, current: 0 }
    }
}

pub struct FChannelIter {
    m: u32,
    n: u32,
    current: u32, // текущая позиция в блоке (0 = начало блока)
}

impl FChannelIter {
    pub fn reset(&mut self) {
        self.current = 0;
    }
}

impl Iterator for FChannelIter {
    type Item = FChannel;

    fn next(&mut self) -> Option<Self::Item> {
        let block_size = self.m + self.n;

        if block_size == 0 {
            // Бесконечная последовательность из P (если n=0) или T (если m=0)
            return Some(if self.m > 0 {
                FChannel::Pressure
            } else {
                FChannel::Temperature
            });
        }

        let pos = self.current % block_size;
        self.current += 1;

        if pos < self.m {
            Some(FChannel::Pressure)
        } else {
            Some(FChannel::Temperature)
        }
    }
}
