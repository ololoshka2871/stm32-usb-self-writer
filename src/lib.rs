#![no_std]
#![no_main]

#![allow(static_mut_refs)]
#![feature(adt_const_params)]

// For allocator
#![feature(alloc_error_handler)]

mod main_data_storage;
mod protobuf;
mod sensors;
mod settings;

pub mod support;
pub mod workmodes;
pub mod config;

extern crate alloc;

use freertos_core as _;

//---------------------------------------------------------------

#[global_allocator]
static GLOBAL: freertos_rust::FreeRtosAllocator = freertos_rust::FreeRtosAllocator;

//---------------------------------------------------------------

#[cfg(debug_assertions)]
mod master_value_stat;

mod threads;

use stm32l4xx_hal::stm32;

use crate::{
    support::{usb_connection_checker::UsbConnectionChecker, vusb_monitor::VUsbMonitor},
    workmodes::WorkMode,
};

pub fn start_at_mode<T>(
    p: cortex_m::Peripherals,
    dp: stm32::Peripherals,
) -> Result<(), freertos_rust::FreeRtosError>
where
    T: WorkMode<T>,
{
    let mut mode = T::new(p, dp);
    mode.ini_static();
    mode.configure_clock();
    mode.print_clock_config();

    #[cfg(debug_assertions)]
    master_value_stat::init_master_getter(
        sensors::freqmeter::master_counter::MasterCounter::acquire(),
    );

    mode.start_threads()
}

pub fn is_usb_connected() -> bool {
    let rcc = unsafe { &*stm32::RCC::ptr() };
    let pwr = unsafe { &*stm32::PWR::ptr() };

    VUsbMonitor::new(rcc, pwr).is_usb_connected()
}

//-----------------------------------------------------------------------------

// defmt-test 0.4.0 has the limitation that this `#[tests]` attribute can only be used
// once within a crate. the module can be in any file but there can only be at most
// one `#[tests]` module in this library crate
#[cfg(test)]
#[defmt_test::tests]
mod unit_tests {
    use defmt::assert;

    #[test]
    fn it_works() {
        assert!(true)
    }
}