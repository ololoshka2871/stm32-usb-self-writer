#![no_std]

extern crate alloc;

pub mod clocking;
pub mod config;
pub mod sensors;
pub mod support;

pub use crate::support::{
    interrupt_controller::IInterruptController, nop_delay::NOPDelay, InputChannel,
    InterruptController,
};

//
//#[cfg(debug_assertions)]
//mod master_value_stat;
//
//use support::{usb_connection_checker::UsbConnectionChecker, vusb_monitor::VUsbMonitor};
//
//pub use crate::{
//    support::free_rtos_error_ext::FreeRtosErrorContainer,
//    workmodes::{
//        high_performance_mode::HighPerformanceMode, recorder_mode::RecorderMode, WorkMode,
//    },
//};
//
//pub fn start_at_mode<T>(
//    p: cortex_m::Peripherals,
//    dp: stm32::Peripherals,
//) -> Result<(), freertos_rust::FreeRtosError>
//where
//    T: WorkMode<T>,
//{
//    let mut mode = T::new(p, dp);
//    mode.ini_static();
//    mode.configure_clock();
//    mode.print_clock_config();
//
//    #[cfg(debug_assertions)]
//    master_value_stat::init_master_getter(
//        sensors::freqmeter::master_counter::MasterCounter::acquire(),
//    );
//
//    mode.start_threads()
//}

pub fn is_usb_connected() -> bool {
    use stm32l4xx_hal::stm32;
    use support::usb_connection_checker::UsbConnectionChecker;

    let rcc = unsafe { &*stm32::RCC::ptr() };
    let pwr = unsafe { &*stm32::PWR::ptr() };

    support::vusb_monitor::VUsbMonitor::new(rcc, pwr).is_usb_connected()
}
