#![no_std]
#![no_main]
// For allocator
#![feature(alloc_error_handler)]
#![feature(adt_const_params)]

extern crate alloc;

use cortex_m_rt::entry;
use stm32l4xx_hal::stm32;

use stm32_usb_self_writer::{
    is_usb_connected, start_at_mode, FreeRtosErrorContainer, HighPerformanceMode, RecorderMode,
};

//---------------------------------------------------------------

#[global_allocator]
static GLOBAL: freertos_rust::FreeRtosAllocator = freertos_rust::FreeRtosAllocator;

//---------------------------------------------------------------

#[entry]
fn main() -> ! {
    // #[cfg(debug_assertions)]
    // cortex_m::asm::bkpt();

    defmt::trace!("++ Start up! ++");

    let p = unsafe { cortex_m::Peripherals::take().unwrap_unchecked() };
    let dp = unsafe { stm32::Peripherals::take().unwrap_unchecked() };

    let start_res = if is_usb_connected() {
        defmt::info!("USB connected, CPU max performance mode");
        start_at_mode::<HighPerformanceMode>(p, dp)
    } else {
        defmt::info!("USB not connected, self-writer mode");
        start_at_mode::<RecorderMode>(p, dp)
    };

    start_res
        .unwrap_or_else(|e| defmt::panic!("Failed to start thread: {}", FreeRtosErrorContainer(e)));

    freertos_rust::FreeRtosUtils::start_scheduler();
}
