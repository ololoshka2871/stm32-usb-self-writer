#![allow(unused)]

/// Get an address and memory size the DMA can use.
///
/// This trait is missing in stm32l4xx-hal
/// 
/// # Safety
///
/// Both the memory size and the address must be correct for the specific peripheral and for the
/// DMA.
pub unsafe trait PeriAddress {
    /// Memory size of the peripheral.
    type MemSize;

    /// Returns the address to be used by the DMA stream.
    fn address(&self) -> u32;
}

/// Marker trait for structs which can be safely accessed with shared reference
/// This trait is missing in stm32l4xx-hal
pub trait SafePeripheralRead {}