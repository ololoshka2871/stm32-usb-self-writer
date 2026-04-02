mod internal;
mod rv3028;
mod rx8130;

#[allow(dead_code)]
use alloc::boxed::Box;
use defmt;
use freertos_rust::{Duration, InterruptContext, Task, TaskNotification, TaskPriority};

use stm32l4xx_hal::{interrupt, stm32};

use crate::rtc::internal::InternalRtc;

#[derive(Copy, Clone, Debug, Default)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub ms: u16,
}

impl DateTime {
    pub fn to_timestamp_ms(&self) -> u64 {
        // Simple conversion to milliseconds since 2000-01-01 00:00:00
        let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut days = (self.year as u64 - 2000) * 365;
        for y in 2000..self.year {
            if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
                days += 1; // leap year
            }
        }
        for m in 1..self.month {
            days += days_in_month[(m - 1) as usize] as u64;
            if m == 2 && ((self.year % 4 == 0 && self.year % 100 != 0) || (self.year % 400 == 0)) {
                days += 1; // leap day
            }
        }
        days += self.day as u64 - 1;
        let hours = days * 24 + self.hour as u64;
        let minutes = hours * 60 + self.minute as u64;
        let seconds = minutes * 60 + self.second as u64;
        seconds * 1000 + self.ms as u64
    }
}

impl defmt::Format for DateTime {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            self.ms
        );
    }
}

/// Universal RTC trait
pub trait Rtc {
    fn set_time(&mut self, dt: DateTime) -> Result<(), ()>;
    fn get_time(&mut self) -> Result<DateTime, ()>;
    /// Enable 1 Hz tick routed to EXTI (PC2). Implementations should configure
    /// the chip to drive the pin and return Ok.
    fn enable_1hz_int(&mut self) -> Result<(), ()>;
}

static mut RTC_INSTANCE: Option<InternalRtc> = None;
static mut EXTERNAL_RTC_INSTANCE: Option<Box<dyn Rtc>> = None;
static mut EXTI2_SYNC_TASK: Option<freertos_rust::Task> = None;
static mut RTC_PERIODIC_TASK: Option<freertos_rust::Task> = None;

pub fn set_global_rtc(r: InternalRtc) {
    unsafe { RTC_INSTANCE = Some(r) }
}

pub fn with_global_rtc<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut dyn Rtc) -> R,
{
    // SAFETY: single-threaded initialization expected during startup.
    unsafe {
        if let Some(ref mut b) = &mut RTC_INSTANCE {
            Some(f(b))
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
    // Always initialize MCU internal RTC first.
    set_global_rtc(InternalRtc::new());

    // Try to enable LSE for internal RTC; fallback to LSI if needed.
    let rcc_regs = unsafe { &*stm32::RCC::ptr() };
    rcc_regs.bdcr.modify(|_, w| w.lseon().set_bit());

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

    // Probe external RTC on I2C and synchronise internal if found.
    let mut buf = [0u8; 1];
    let external_detected = if i2c
        .write_read(rv3028::RV3028_I2C_ADDR, &[0x00], &mut buf)
        .is_ok()
    {
        defmt::info!("Found external RTC: RV-3028");
        unsafe {
            EXTERNAL_RTC_INSTANCE = Some(Box::new(rv3028::Rv3028::new(i2c)));
        }
        true
    } else if i2c
        .write_read(rx8130::RX8130_I2C_ADDR, &[0x00], &mut buf)
        .is_ok()
    {
        defmt::info!("Found external RTC: RX8130");
        unsafe {
            EXTERNAL_RTC_INSTANCE = Some(Box::new(rx8130::Rx8130::new(i2c)));
        }
        true
    } else {
        defmt::warn!("No external RTC found, using internal STM32 RTC");
        drop(i2c);
        false
    };

    // Ensure internal RTC has at least 1Hz event if only internal is available.
    if external_detected {
        if let Some(ext_rtc) = unsafe { EXTERNAL_RTC_INSTANCE.as_mut() } {
            if let Ok(mut ext_time) = ext_rtc.get_time() {
                ext_time.ms = 0; // external RTC has no software subsecond field
                if let Some(Ok(_)) = with_global_rtc(|r| r.set_time(ext_time)) {
                    defmt::info!("Internal RTC set from external RTC time");
                }
            }

            let _ = ext_rtc.enable_1hz_int();

            enable_exti2();
            if let Err(_) = start_rtc_sync_task() {
                defmt::warn!("Failed to start RTC sync task");
            }
        }
    }
}

/// Convenience wrappers
pub fn _rtc_set_time(dt: DateTime) -> Result<(), ()> {
    with_global_rtc(|r| r.set_time(dt)).unwrap_or(Err(()))
}

pub fn rtc_get_time() -> DateTime {
    with_global_rtc(|r| r.get_time())
        .unwrap_or(Ok(DateTime::default()))
        .unwrap_or_default()
}

pub fn rtc_set_alarm_periodic(period_ms: u32) -> Result<(), ()> {
    use cortex_m::peripheral::NVIC;

    if period_ms == 0 {
        return Err(());
    }

    let current_task = Task::current().map_err(|_| ())?;
    unsafe {
        RTC_PERIODIC_TASK = Some(current_task);
    }

    let rtc = unsafe { &*stm32::RTC::ptr() };
    let exti = unsafe { &*stm32::EXTI::ptr() };

    let rtc_clk_hz: u32 = {
        let rcc = unsafe { &*stm32::RCC::ptr() };
        if rcc.bdcr.read().rtcsel().bits() == 0b01 {
            32_768
        } else {
            32_000
        }
    };

    // WUCKSEL = 0b000 => RTCCLK / 16
    let wakeup_clk_hz = rtc_clk_hz / 16;
    let ticks = ((period_ms as u64 * wakeup_clk_hz as u64 + 999) / 1000)
        .clamp(1, 0x1_0000) as u32;

    // Unlock RTC write protection
    rtc.wpr.write(|w| unsafe { w.bits(0xCA) });
    rtc.wpr.write(|w| unsafe { w.bits(0x53) });

    rtc.cr.modify(|_, w| {
        w.wute().clear_bit();
        w.wutie().clear_bit()
    });
    while rtc.isr.read().wutwf().bit_is_clear() {}

    rtc.cr.modify(|_, w| unsafe { w.wucksel().bits(0b000) });
    rtc.wutr.write(|w| unsafe { w.wut().bits((ticks - 1) as u16) });
    rtc.isr.modify(|_, w| w.wutf().clear_bit());

    // Route wakeup event to EXTI20
    exti.imr1.modify(|_, w| w.mr20().set_bit());
    exti.rtsr1.modify(|_, w| w.tr20().set_bit());
    exti.pr1.write(|w| w.pr20().set_bit());

    rtc.cr.modify(|_, w| {
        w.wutie().set_bit();
        w.wute().set_bit()
    });

    // Re-lock write protection
    rtc.wpr.write(|w| unsafe { w.bits(0xFF) });

    unsafe {
        let mut nvic = cortex_m::Peripherals::steal().NVIC;
        nvic.set_priority(stm32::Interrupt::RTC_WKUP, crate::config::USB_INTERRUPT_PRIO);
        NVIC::unmask(stm32::Interrupt::RTC_WKUP);
    }

    Ok(())
}

pub fn rtc_wait_periodic_tick() -> Result<(), ()> {
    let task = Task::current().map_err(|_| ())?;
    let _ = task.take_notification(true, Duration::infinite());
    Ok(())
}

fn sync_external_to_internal() {
    if let Some(ext_rtc) = unsafe { EXTERNAL_RTC_INSTANCE.as_mut() } {
        if let Ok(mut ext_time) = ext_rtc.get_time() {
            // external devices don't provide subsecond, keep 0
            ext_time.ms = 0;
            if let Some(Ok(_)) = with_global_rtc(|r| r.set_time(ext_time)) {
                defmt::info!("RTC synced from external RTC to internal RTC");
            } else {
                defmt::warn!("Internal RTC set_time failed during sync");
            }
        } else {
            defmt::warn!("Failed to read time from external RTC during sync");
        }
    }
}

pub fn start_rtc_sync_task() -> Result<(), freertos_rust::FreeRtosError> {
    let task = Task::new()
        .name("RtcSync")
        .stack_size(512)
        .priority(TaskPriority(2))
        .start(|_| {
            let mut tick_cnt: u8 = 0;
            loop {
                let _ = unsafe { freertos_rust::Task::current().unwrap_unchecked() }
                    .take_notification(true, Duration::infinite());
                tick_cnt = tick_cnt.saturating_add(1);
                if tick_cnt >= 60 {
                    tick_cnt = 0;
                    sync_external_to_internal();
                }
            }
        })?;

    unsafe {
        EXTI2_SYNC_TASK = Some(task);
    }

    Ok(())
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

        // FreeRTOS-safe ISR priority for task notification from ISR.
        let mut nvic = cortex_m::Peripherals::steal().NVIC;
        nvic.set_priority(stm32::Interrupt::EXTI2, crate::config::USB_INTERRUPT_PRIO);

        // enable NVIC for EXTI2
        NVIC::unmask(stm32::Interrupt::EXTI2);
    }
}

//-----------------------------------------------------------------------------

#[allow(non_snake_case)]
#[allow(unused)]
#[cortex_m_rt::interrupt]
fn EXTI2() {
    defmt::info!("RTC 1Hz tick (EXTI2)");

    // Notify sync task; can be missing (e.g. only internal RTC mode).
    let interrupt_ctx = InterruptContext::new();
    unsafe {
        if let Some(task) = EXTI2_SYNC_TASK.as_ref() {
            let _ = task.notify_from_isr(&interrupt_ctx, TaskNotification::Increment);
        }
    }

    // Clear EXTI2 pending bit.
    let exti = unsafe { &*stm32::EXTI::ptr() };
    exti.pr1.write(|w| w.pr2().set_bit());
}

#[allow(non_snake_case)]
#[allow(unused)]
#[cortex_m_rt::interrupt]
fn RTC_WKUP() {
    let interrupt_ctx = InterruptContext::new();
    unsafe {
        if let Some(task) = RTC_PERIODIC_TASK.as_ref() {
            let _ = task.notify_from_isr(&interrupt_ctx, TaskNotification::Increment);
        }
    }

    let rtc = unsafe { &*stm32::RTC::ptr() };
    let exti = unsafe { &*stm32::EXTI::ptr() };

    rtc.isr.modify(|_, w| w.wutf().clear_bit());
    exti.pr1.write(|w| w.pr20().set_bit());
}

//-----------------------------------------------------------------------------

pub fn dec2bcd(val: u8) -> u8 {
    ((val / 10) << 4) | (val % 10)
}
pub fn bcd2dec(val: u8) -> u8 {
    ((val >> 4) * 10) + (val & 0x0F)
}
