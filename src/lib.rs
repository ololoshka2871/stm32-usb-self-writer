#![no_std]
#![feature(adt_const_params)]

extern crate alloc;

pub mod clocking;
pub mod config;
pub mod protobuf;
pub mod qspi_storage;
pub mod main_data_storage;
pub mod sensors;
pub mod settings;
pub mod support;
pub mod vfs;
pub mod workmodes;

pub use crate::support::{
    InputChannel, InterruptController, interrupt_controller::IInterruptController,
    nop_delay::NOPDelay, power_ctrl::PowerCtrl, rtc_sync::RtcSync,
};

pub fn is_usb_connected() -> bool {
    use stm32l4xx_hal::stm32;
    use support::usb_connection_checker::UsbConnectionChecker;

    let rcc = unsafe { &*stm32::RCC::ptr() };
    let pwr = unsafe { &*stm32::PWR::ptr() };

    support::vusb_monitor::VUsbMonitor::new(rcc, pwr).is_usb_connected()
}
