mod internal;
mod rv3028;
mod rx8130;

#[allow(dead_code)]
use alloc::boxed::Box;
use defmt;

use stm32l4xx_hal::{interrupt, stm32};

#[derive(Copy, Clone, Debug, Default)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// Universal RTC trait
pub trait Rtc {
    fn set_time(&mut self, dt: DateTime) -> Result<(), ()>;
    fn get_time(&mut self) -> Result<DateTime, ()>;
    /// Enable 1 Hz tick routed to EXTI (PC2). Implementations should configure
    /// the chip to drive the pin and return Ok.
    fn enable_1hz_exti(&mut self) -> Result<(), ()>;
}

static mut RTC_INSTANCE: Option<Box<dyn Rtc>> = None;

pub fn set_global_rtc(r: Box<dyn Rtc>) {
    unsafe { RTC_INSTANCE = Some(r) }
}

pub fn with_global_rtc<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut dyn Rtc) -> R,
{
    // SAFETY: single-threaded initialization expected during startup.
    unsafe {
        if let Some(ref mut b) = &mut RTC_INSTANCE {
            Some(f(b.as_mut()))
        } else {
            None
        }
    }
}

/// Initialize RTC subsystem: probe RV-3028 at 0x56 and RX8130 at 0x32 on given I2C.
/// If none found, fall back to STM32 internal RTC (best-effort stub here).
///
/// This function is intentionally generic in the I2C type and in pin types so it
/// can be called from both workmodes after pins/peripherals are available.
pub fn init<I2C, E>(mut i2c: I2C)
where
    I2C: embedded_hal::blocking::i2c::WriteRead<Error = E>
        + embedded_hal::blocking::i2c::Write<Error = E>
        + 'static,
{
    enum RtcBackendKind {
        Rv3028,
        Rx8130,
        Internal,
    }

    // Try RV-3028 (0x56)
    let mut buf = [0u8; 1];
    let found: RtcBackendKind = if i2c
        .write_read(rv3028::RV3028_I2C_ADDR, &[0x00], &mut buf)
        .is_ok()
    {
        defmt::info!("Found RTC: RV-3028");
        RtcBackendKind::Rv3028
    } else if i2c
        .write_read(rx8130::RX8130_I2C_ADDR, &[0x00], &mut buf)
        .is_ok()
    {
        defmt::info!("Found RTC: RX8130");
        RtcBackendKind::Rx8130
    } else {
        defmt::warn!("No external RTC found, will use internal STM32 RTC");
        RtcBackendKind::Internal
    };

    match found {
        RtcBackendKind::Rv3028 => {
            set_global_rtc(Box::new(rv3028::Rv3028::new(i2c)));
            enable_exti2();
        }
        RtcBackendKind::Rx8130 => {
            set_global_rtc(Box::new(rx8130::Rx8130::new(i2c)));
            enable_exti2();
        }
        RtcBackendKind::Internal => {
            // Fallback: try to enable internal RTC. Best-effort minimal init so
            // device has a usable RTC. Full functionality can be expanded later.
            set_global_rtc(Box::new(internal::InternalRtc));

            // Try to enable LSI for RTC
            let rcc_regs = unsafe { &*stm32::RCC::ptr() };
            
            // Try to enable LSE first
            rcc_regs.bdcr.modify(|_, w| w.lseon().set_bit());
            // Wait for LSE ready or timeout (~100ms)
            let mut lse_ready = false;
            for _ in 0..100_000 {
                if rcc_regs.bdcr.read().lserdy().bit_is_set() {
                    lse_ready = true;
                    break;
                }
            }
            if lse_ready {
                defmt::info!("Internal RTC: LSE enabled");
            } else {
                defmt::warn!("Internal RTC: LSE failed, falling back to LSI");
                rcc_regs.csr.modify(|_, w| w.lsion().set_bit());
                while rcc_regs.csr.read().lsirdy().bit_is_clear() {}
                defmt::info!("Internal RTC: LSI enabled");
            }

            // Configure internal RTC (1Hz wakeup) through its trait implementation.
            if let Some(_) = with_global_rtc(|r| r.enable_1hz_exti().ok()) {
                defmt::info!("Internal RTC: enable_1hz_exti called");
            }

            // deinitialize i2c to free up pins/peripheral for other use since internal RTC doesn't need it
            drop(i2c);
        }
    }
}

/// Convenience wrappers
pub fn rtc_set_time(dt: DateTime) -> Result<(), ()> {
    with_global_rtc(|r| r.set_time(dt)).unwrap_or(Err(()))
}

pub fn rtc_get_time() -> Result<DateTime, ()> {
    with_global_rtc(|r| r.get_time()).unwrap_or(Err(()))
}

/// Minimal helper to route PC2 to EXTI line 2 and enable the EXTI2_3 NVIC.
pub fn enable_exti2() {
    use cortex_m::peripheral::NVIC;
    use stm32l4xx_hal::stm32;

    let syscfg = unsafe { &*stm32::SYSCFG::ptr() };
    let exti = unsafe { &*stm32::EXTI::ptr() };

    // Map EXTI2 to port C (value 0b10)
    // EXTICR1 holds EXTI0..3 mapping
    unsafe {
        // write 4-bit field for EXTI2
        let v = syscfg.exticr1.read().bits();
        // clear bits 8..11 and set to 0b0010 (port C)
        let v = (v & !(0xF << 8)) | ((0b0010u32 & 0xF) << 8);
        syscfg.exticr1.write(|w| w.bits(v));

        // unmask EXTI2
        exti.imr1.modify(|_, w| w.mr2().set_bit());
        // enable rising trigger
        exti.rtsr1.modify(|_, w| w.tr2().set_bit());

        // enable NVIC for EXTI2
        NVIC::unmask(stm32::Interrupt::EXTI2);
    }
}

#[allow(non_snake_case)]
#[allow(unused)]
#[cortex_m_rt::interrupt]
fn EXTI2() {
    // Clear pending bits where necessary — keep minimal here and just log.
    defmt::info!("RTC 1Hz tick (EXTI2)");

    // Clearing EXTI flags can be added later if needed.
    let exti = unsafe { &*stm32::EXTI::ptr() };
    exti.pr1.write(|w| w.pr2().set_bit());
}

#[allow(non_snake_case)]
#[allow(unused)]
#[cortex_m_rt::interrupt]
fn RTC_WKUP() {
    defmt::info!("RTC internal 1Hz tick (RTC_WKUP)");

    // clear WUT flag if set
    let rtc = unsafe { &*stm32::RTC::ptr() };
    rtc.isr.modify(|_, w| w.wutf().clear_bit());
}

//-----------------------------------------------------------------------------

pub fn dec2bcd(val: u8) -> u8 {
    ((val / 10) << 4) | (val % 10)
}
pub fn bcd2dec(val: u8) -> u8 {
    ((val >> 4) * 10) + (val & 0x0F)
}
