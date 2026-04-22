//pub mod high_performance_mode;
//pub mod recorder_mode;

//pub mod common;
//mod my_clock_freeze;

pub mod output_storage;
//pub mod processing;

#[derive(Clone, Copy, Debug, PartialEq, defmt::Format)]
#[repr(u8)]
pub enum FChannel {
    Pressure = 0,
    Temperature = 1,

    Both = 222,
}

impl FChannel {
    pub fn iter(m: u32, n: u32) -> FChannelIter {
        assert_ne!(m, 0);
        assert_ne!(n, 0);
        FChannelIter {
            m,
            n,
            current_m: m - 1,
            current_n: n - 1,
        }
    }
}

pub struct FChannelIter {
    m: u32,
    n: u32,
    current_m: u32,
    current_n: u32,
}

impl FChannelIter {
    pub fn reset(&mut self) {
        self.current_m = self.m - 1;
        self.current_n = self.n - 1;
    }
}

impl Iterator for FChannelIter {
    type Item = FChannel;

    fn next(&mut self) -> Option<Self::Item> {
        self.current_m += 1;
        self.current_n += 1;

        if self.current_m == self.m && self.current_n == self.n {
            self.current_m = 0;
            self.current_n = 0;
            Some(FChannel::Both)
        } else if self.current_m == self.m {
            self.current_m = 0;
            Some(FChannel::Pressure)
        } else if self.current_n == self.n {
            self.current_n = 0;
            Some(FChannel::Temperature)
        } else {
            None
        }
    }
}

pub trait ChannelChecker {
    fn check(&self, channel: FChannel) -> bool;
}

impl ChannelChecker for Option<FChannel> {
    fn check(&self, channel: FChannel) -> bool {
        match self {
            Some(c) => *c == channel,
            None => false,
        }
    }
}
