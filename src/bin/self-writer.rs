#![no_std]
#![no_main]

use cortex_m_rt::entry;

use stm32l4xx_hal::stm32;

use stm32_usb_self_writer::{
    support::free_rtos_error_ext::FreeRtosErrorContainer,
    workmodes::{HighPerformanceMode, RecorderMode},
};

#[entry]
fn main() -> ! {
    // #[cfg(debug_assertions)]
    // cortex_m::asm::bkpt();

    defmt::trace!("++ Start up! ++");

    let p = unsafe { cortex_m::Peripherals::take().unwrap_unchecked() };
    let dp = unsafe { stm32::Peripherals::take().unwrap_unchecked() };

    let start_res = if stm32_usb_self_writer::is_usb_connected() {
        defmt::info!("USB connected, CPU max performance mode");
        stm32_usb_self_writer::start_at_mode::<HighPerformanceMode>(p, dp)
    } else {
        defmt::info!("USB not connected, self-writer mode");
        stm32_usb_self_writer::start_at_mode::<RecorderMode>(p, dp)
    };

    start_res
        .unwrap_or_else(|e| defmt::panic!("Failed to start thread: {}", FreeRtosErrorContainer(e)));

    freertos_rust::FreeRtosUtils::start_scheduler();
}
