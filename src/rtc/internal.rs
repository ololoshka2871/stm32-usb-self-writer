use stm32l4xx_hal::stm32;

use super::{DateTime, Rtc, dec2bcd, bcd2dec};

pub struct InternalRtc;

fn ensure_rtc_enabled() -> Result<(), ()> {
    let rcc = unsafe { &*stm32::RCC::ptr() };
    let pwr = unsafe { &*stm32::PWR::ptr() };
    let rtc = unsafe { &*stm32::RTC::ptr() };

    // Enable PWR interface and backup domain access
    rcc.apb1enr1.modify(|_, w| w.pwren().set_bit());
    pwr.cr1.modify(|_, w| w.dbp().set_bit());
    while pwr.cr1.read().dbp().bit_is_clear() {}

    // If RTC was not enabled, initialize backup domain and RTC clock source.
    if rcc.bdcr.read().rtcen().bit_is_clear() {
        // reset and release backup domain
        rcc.bdcr.modify(|_, w| w.bdrst().set_bit());
        rcc.bdcr.modify(|_, w| w.bdrst().clear_bit());

        // choose RTC clock source 
        if rcc.bdcr.read().lserdy().bit_is_set() {
            rcc.bdcr
                .modify(|_, w| unsafe { w.rtcsel().bits(0b01) }); // LSE
            defmt::info!("Internal RTC: LSE selected as clock source");
        } else {
            rcc.bdcr
                .modify(|_, w| unsafe { w.rtcsel().bits(0b10) }); // LSI
            defmt::warn!("Internal RTC: LSE not ready, LSI selected as clock source");
        }

        rcc.bdcr.modify(|_, w| w.rtcen().set_bit());
    }

    // Unlock RTC write protection
    rtc.wpr.write(|w| unsafe { w.bits(0xCA) });
    rtc.wpr.write(|w| unsafe { w.bits(0x53) });

    // Enter initialization mode and set prescalers to produce 1Hz.
    rtc.isr.modify(|_, w| w.init().set_bit());
    while rtc.isr.read().initf().bit_is_clear() {}

    rtc.prer.write(|w| unsafe {
        w.prediv_a().bits(127); // Asynchronous prescaler
        w.prediv_s().bits(255) // Synchronous prescaler
    });

    rtc.isr.modify(|_, w| w.init().clear_bit());
    while rtc.isr.read().initf().bit_is_set() {}

    Ok(())
}

fn wait_rtc_sync() {
    let rtc = unsafe { &*stm32::RTC::ptr() };
    // Force synchronization with shadow registers
    rtc.isr.modify(|_, w| w.rsf().clear_bit());
    while rtc.isr.read().rsf().bit_is_clear() {}
}

impl Rtc for InternalRtc {
    fn set_time(&mut self, dt: DateTime) -> Result<(), ()> {
        ensure_rtc_enabled()?;

        let rtc = unsafe { &*stm32::RTC::ptr() };

        // Unlock RTC write protection
        rtc.wpr.write(|w| unsafe { w.bits(0xCA) });
        rtc.wpr.write(|w| unsafe { w.bits(0x53) });

        rtc.isr.modify(|_, w| w.init().set_bit());
        while rtc.isr.read().initf().bit_is_clear() {}

        rtc.tr.write(|w| unsafe {
            w.pm().clear_bit();
            w.ht().bits((dt.hour / 10) as u8);
            w.hu().bits((dt.hour % 10) as u8);
            w.mnt().bits((dt.minute / 10) as u8);
            w.mnu().bits((dt.minute % 10) as u8);
            w.st().bits((dt.second / 10) as u8);
            w.su().bits((dt.second % 10) as u8)
        });

        let year = dt.year.saturating_sub(2000);
        rtc.dr.write(|w| unsafe {
            w.yt().bits((year / 10) as u8);
            w.yu().bits((year % 10) as u8);
            w.wdu().bits(1); // weekday placeholder
            w.mt().bit(dt.month >= 10);
            w.mu().bits((dt.month % 10) as u8);
            w.dt().bits((dt.day / 10) as u8);
            w.du().bits((dt.day % 10) as u8)
        });

        rtc.isr.modify(|_, w| w.init().clear_bit());
        while rtc.isr.read().initf().bit_is_set() {}

        Ok(())
    }

    fn get_time(&mut self) -> Result<DateTime, ()> {
        ensure_rtc_enabled()?;

        let rtc = unsafe { &*stm32::RTC::ptr() };
        wait_rtc_sync();

        let tr = rtc.tr.read();
        let dr = rtc.dr.read();

        let seconds = bcd2dec(tr.st().bits() * 10 + tr.su().bits());
        let minutes = bcd2dec(tr.mnt().bits() * 10 + tr.mnu().bits());
        let hours = bcd2dec(tr.ht().bits() * 10 + tr.hu().bits());

        let day = bcd2dec(dr.dt().bits() * 10 + dr.du().bits());
        let month = bcd2dec((dr.mt().bit() as u8) * 10 + dr.mu().bits());
        let year = 2000 + bcd2dec(dr.yt().bits() * 10 + dr.yu().bits()) as u16;

        Ok(DateTime {
            year,
            month,
            day,
            hour: hours,
            minute: minutes,
            second: seconds,
        })
    }

    fn enable_1hz_exti(&mut self) -> Result<(), ()> {
        ensure_rtc_enabled()?;

        let rtc = unsafe { &*stm32::RTC::ptr() };

        // Unlock RTC write protection
        rtc.wpr.write(|w| unsafe { w.bits(0xCA) });
        rtc.wpr.write(|w| unsafe { w.bits(0x53) });

        // disable wakeup timer before configuration
        rtc.cr.modify(|_, w| w.wute().clear_bit());
        while rtc.isr.read().wutf().bit_is_set() {
            rtc.isr.modify(|_, w| w.wutf().clear_bit());
        }
        while rtc.isr.read().wutwf().bit_is_clear() {}

        // set auto-reload for 1 second
        rtc.wutr.write(|w| unsafe { w.wut().bits(1) });

        // select ck_spre (1 Hz) as wakeup clock
        rtc.cr.modify(|_, w| unsafe { w.wucksel().bits(0b100) });

        rtc.cr.modify(|_, w| w.wutie().set_bit());
        rtc.cr.modify(|_, w| w.wute().set_bit());

        unsafe {
            cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::RTC_WKUP);
        }

        Ok(())
    }
}

