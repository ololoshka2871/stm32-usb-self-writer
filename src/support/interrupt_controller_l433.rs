use core::cell::RefCell;

use super::interrupt_controller::{self, Interrupt};
use cortex_m::{interrupt::InterruptNumber, peripheral::NVIC};
use stm32l4xx_hal::stm32l4::stm32l4x3::Interrupt as IRQ;

pub struct InterruptController(RefCell<cortex_m::peripheral::NVIC>);

unsafe impl Sync for InterruptController {}
unsafe impl Send for InterruptController {}

impl InterruptController {
    pub fn new(nvic: cortex_m::peripheral::NVIC) -> Self {
        Self(RefCell::new(nvic))
    }
}

impl interrupt_controller::IInterruptController for InterruptController {
    fn set_priority(&self, interrupt: interrupt_controller::Interrupt, prio: u8) {
        unsafe { self.0.borrow_mut().set_priority(interrupt, prio) };
    }

    fn unmask(&self, interrupt: interrupt_controller::Interrupt) {
        unsafe { NVIC::unmask(interrupt) };
    }

    fn mask(&self, interrupt: interrupt_controller::Interrupt) {
        NVIC::mask(interrupt);
    }

    fn unpend(&self, interrupt: interrupt_controller::Interrupt) {
        NVIC::unpend(interrupt);
    }

    fn is_pending(&self, interrupt: Interrupt) -> bool {
        NVIC::is_pending(interrupt)
    }
}

impl Into<Interrupt> for IRQ {
    fn into(self) -> Interrupt {
        Interrupt { 0: self.number() }
    }
}
