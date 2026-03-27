#![no_std]
#![allow(static_mut_refs)]
// For allocator
#![feature(alloc_error_handler)]
#![feature(adt_const_params)]

extern crate alloc;

mod main_data_storage;
mod protobuf;
mod sensors;
mod settings;
mod support;
mod threads;

pub mod workmodes;
pub mod config;

#[cfg(debug_assertions)]
mod master_value_stat;

use stm32l4xx_hal::stm32;
use support::{usb_connection_checker::UsbConnectionChecker, vusb_monitor::VUsbMonitor};

pub use crate::{
    support::free_rtos_error_ext::FreeRtosErrorContainer,
    workmodes::{
        high_performance_mode::HighPerformanceMode, recorder_mode::RecorderMode, WorkMode,
    },
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

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "C" fn SysTick() {
    use rtic_monotonics::TimerQueueBackend;
    rtic_monotonics::systick::SystickBackend::timer_queue().on_monotonic_interrupt();
}

#[macro_export]
macro_rules! define_self_writer_monotonic {
    ($name:ident, $rate_hz:expr) => {
        pub struct $name;

        impl $name {
            pub fn start(systick: rtic_monotonics::systick::SYST, sysclk: u32) {
                rtic_monotonics::systick::SystickBackend::_start(systick, sysclk, $rate_hz);
            }
        }

        impl rtic_monotonics::TimerQueueBasedMonotonic for $name {
            type Backend = rtic_monotonics::systick::SystickBackend;
            type Instant = rtic_monotonics::fugit::Instant<
                <Self::Backend as rtic_monotonics::TimerQueueBackend>::Ticks,
                1,
                $rate_hz,
            >;
            type Duration = rtic_monotonics::fugit::Duration<
                <Self::Backend as rtic_monotonics::TimerQueueBackend>::Ticks,
                1,
                $rate_hz,
            >;
        }

        rtic_monotonics::rtic_time::impl_embedded_hal_delay_fugit!($name);
        rtic_monotonics::rtic_time::impl_embedded_hal_async_delay_fugit!($name);
    };
}
