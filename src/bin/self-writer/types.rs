#![allow(dead_code)]

#[allow(unused_imports)]
use stm32l4xx_hal::gpio::{
    Alternate, Analog, Output, PA0, PA1, PA2, PA3, PA6, PA7, PA8, PB0, PB1, PC10, PD10, PD11, PD13,
    PE12, PushPull,
};

use stm32_usb_self_writer::{clocking::*, config, workmodes::FChannel};

//-----------------------------------------------------------------------------

pub struct Pll;

#[cfg(feature = "xtal-24mhz")]
impl PllConfigProvider for Pll {
    const PD: u32 = 3;
    const M: u32 = 20;
    const AD: u32 = 2;

    const SAI_MUL: u32 = 12;
    const SAI_DIV_CODE: u32 = 2;
}

#[cfg(feature = "xtal-12mhz")]
impl PllConfigProvider for Pll {
    const PD: u32 = 3;
    const M: u32 = 40;
    const AD: u32 = 2;

    const SAI_MUL: u32 = 24;
    const SAI_DIV_CODE: u32 = 2;
}

pub type HighPerformanceClockProvider =
    HighPerformanceClockConfigProvider<Pll, { config::XTAL_FREQ }, { config::HIGH_PERF_CPU_FREQ }>;
pub type RecorderClockProvider =
    RecorderClockConfigProvider<{ config::XTAL_FREQ }, { config::SELF_WRITER_CPU_FREQ }>;

//-----------------------------------------------------------------------------

#[cfg(feature = "no-flash")]
pub type Flash1 = ();
#[cfg(feature = "no-flash")]
pub type FlashResetPin = ();

#[cfg(feature = "maket")]
pub type Flash1Io1 = PE12<Alternate<PushPull, 10>>;
#[cfg(not(feature = "maket"))]
pub type Flash1Io1 = PB1<Alternate<PushPull, 10>>;

#[cfg(not(feature = "no-flash"))]
pub type FlashResetPin = PD11<Output<PushPull>>;

pub type Led = PC10<Output<PushPull>>;

//-----------------------------------------------------------------------------

pub type VBatPin = PA1<Analog>;
pub type InPPin = PA8<Alternate<PushPull, 1>>;
pub type InTPin = PA0<Alternate<PushPull, 1>>;
pub type EnPPin = PD13<Output<PushPull>>;
pub type EnTPin = PD10<Output<PushPull>>;

//-----------------------------------------------------------------------------

pub type MasterCounter =
    stm32_usb_self_writer::sensors::freqmeter::master_counter::m_tim6::MasterCounter16;
pub type MasterCounterType =
    stm32_usb_self_writer::sensors::freqmeter::master_counter::m_tim6::Type;

//-----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, defmt::Format)]
pub enum FData {
    Pressure(f64),
    Temperature(f64),
    Both(f64, f64),
}

impl FData {
    pub fn as_channel(&self) -> FChannel {
        match self {
            FData::Pressure(_) => FChannel::Pressure,
            FData::Temperature(_) => FChannel::Temperature,
            FData::Both(_, _) => FChannel::Both,
        }
    }
}

pub type DatItem = FData;
