#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use core::alloc::Layout;

use cortex_m_rt::{ExceptionFrame, exception};
use freertos_rust::{FreeRtosCharPtr, FreeRtosTaskHandle};


#[exception]
unsafe fn DefaultHandler(irqn: i16) {
    // custom default handler
    // irqn is negative for Cortex-M exceptions
    // irqn is positive for device specific (line IRQ)
    defmt::panic!("Unregistred irq: {}", irqn);
}

#[exception]
unsafe fn HardFault(_ef: &ExceptionFrame) -> ! {
    defmt::panic!("Hard Fault");
}


// define what happens in an Out Of Memory (OOM) condition
#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    defmt::panic!("Heap allocation failed");
}

#[no_mangle]
fn vApplicationStackOverflowHook(_pxTask: FreeRtosTaskHandle, pcTaskName: FreeRtosCharPtr) {
    defmt::panic!("Thread {} stack overflow detected!", pcTaskName);
}

#[no_mangle]
fn vApplicationMallocFailedHook() {
    defmt::panic!("malloc() failed");
}