#![no_std]
#![no_main]

use stm32_usb_self_writer as _;

#[cfg(test)]
#[defmt_test::tests]
mod tests {
    use super::*;

    #[test]
    fn simple_test() {
        defmt::info!("Running simple test");
        defmt::assert!(2 + 2 == 4, "Math is broken!");
    }
}
