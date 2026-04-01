use stm32l4xx_hal::stm32;

use super::{DateTime, Rtc};

pub struct InternalRtc<L> {
    led: L,
}

impl<L: embedded_hal::digital::v2::OutputPin> InternalRtc<L> {
    pub fn new(led: L) -> Self {
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
                rcc.bdcr.modify(|_, w| unsafe { w.rtcsel().bits(0b01) }); // LSE
            } else {
                rcc.bdcr.modify(|_, w| unsafe { w.rtcsel().bits(0b10) }); // LSI
            }

            rcc.bdcr.modify(|_, w| w.rtcen().set_bit());
        }

        // Unlock RTC write protection
        rtc.wpr.write(|w| unsafe { w.bits(0xCA) });
        rtc.wpr.write(|w| unsafe { w.bits(0x53) });

        // Enter initialization mode and set prescalers to produce 1Hz.
        rtc.isr.modify(|_, w| w.init().set_bit());
        while rtc.isr.read().initf().bit_is_clear() {}

        // Set WUCKSEL to feed from 32768Hz / 16 (default)
        rtc.cr.modify(|_, w| unsafe { w.wucksel().bits(0b000) });

        rtc.isr.modify(|_, w| w.init().clear_bit());
        while rtc.isr.read().initf().bit_is_set() {}

        Self { led }
    }

    fn wait_rtc_sync(&self) {
        let rtc = unsafe { &*stm32::RTC::ptr() };
        // Force synchronization with shadow registers.
        // Must be done on each call to get accurate TR/DR values.
        rtc.isr.modify(|_, w| w.rsf().clear_bit());
        while rtc.isr.read().rsf().bit_is_clear() {}
    }
}

impl<L> Rtc for InternalRtc<L>
where
    L: embedded_hal::digital::v2::OutputPin,
{
    fn set_time(&mut self, dt: DateTime) -> Result<(), ()> {
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
        let rtc = unsafe { &*stm32::RTC::ptr() };

        self.led.set_high().ok(); // indicate RTC read start

        // Synchronize shadow registers from RTC domain before reading.
        self.wait_rtc_sync();

        let mut tr;
        let mut dr;
        let mut subsec;

        loop {
            tr = rtc.tr.read();
            dr = rtc.dr.read();
            subsec = rtc.ssr.read().ss().bits() as u32;

            let tr2 = rtc.tr.read();
            let dr2 = rtc.dr.read();

            if tr.bits() == tr2.bits() && dr.bits() == dr2.bits() {
                break;
            }

            // If values changed while reading, resync and retry.
            self.wait_rtc_sync();
        }

        self.led.set_low().ok(); // indicate RTC read end

        let second = tr.st().bits() * 10 + tr.su().bits();
        let minute = tr.mnt().bits() * 10 + tr.mnu().bits();
        let hour = tr.ht().bits() * 10 + tr.hu().bits();

        let day = dr.dt().bits() * 10 + dr.du().bits();
        let month = (dr.mt().bit() as u8) * 10 + dr.mu().bits();
        let year = 2000 + (dr.yt().bits() * 10 + dr.yu().bits()) as u16;

        let prediv_s = rtc.prer.read().prediv_s().bits() as u32;
        let ms = if prediv_s > 0 {
            (((prediv_s - subsec) * 1000) / (prediv_s + 1)) as u16
        } else {
            0
        };

        Ok(DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            ms,
        })
    }

    fn enable_1hz_exti(&mut self) -> Result<(), ()> {
        unimplemented!("Internal RTC: EXTI output not implemented");
    }
}
