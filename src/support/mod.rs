pub mod filter;
pub mod hex_slice;
pub mod interrupt_controller;
pub mod led;
pub mod len_in_u64_aligned;
pub mod nop_delay;
pub mod power_ctrl;
pub mod rtc_sync;
pub mod usb_connection_checker;
pub mod vusb_monitor;

#[cfg(feature = "stm32l433")]
mod interrupt_controller_l433;

#[cfg(feature = "stm32l433")]
pub use interrupt_controller_l433::InterruptController;

#[cfg(debug_assertions)]
pub mod debug_mcu;

#[derive(Clone, Copy, PartialEq, defmt::Format)]
#[allow(dead_code)]
pub enum InputChannel {
    Ch1 = 0,
    Ch2 = 1,

    COUNT = 2,
}
