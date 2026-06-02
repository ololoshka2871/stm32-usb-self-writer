use stm32l4xx_hal::{
    datetime::{Date, Hour, Micros, Minute, Second, Time, U32Ext},
    gpio::{Alternate, PB2, PC13, PushPull},
    hal::timer::CountDown,
    pac::{self},
    pwr,
    rcc::{APB1R1, BDCR},
    rtc::{Event, Rtc, RtcClockSource, RtcConfig, RtcWakeupClockSource},
    time::Hertz,
};

use super::{CurrentTime, RtcCalibrationOutput, RtcCalibrationOutputPin};

const RTC_INIT_MARKER: u32 = 0xA5A5_5A5A;
const LSE_STARTUP_TIMEOUT_CYCLES: usize = 200_000;
const LSI_STARTUP_TIMEOUT_CYCLES: usize = 200_000;

pub struct RtcService {
    rtc: Rtc,
    rtc_clock_source: RtcClockSource,
    wakeup_ticks: Option<u32>,
}

impl RtcService {
    pub fn init(
        rtc: pac::RTC,
        exti: &mut pac::EXTI,
        apb1r1: &mut APB1R1,
        bdcr: &mut BDCR,
        pwrcr1: &mut pwr::CR1,
    ) -> (Self, RtcClockSource) {
        let source = select_rtc_clock_source();

        let rtc_config = match source {
            RtcClockSource::LSE => RtcConfig::default()
                .clock_config(RtcClockSource::LSE)
                .wakeup_clock_config(RtcWakeupClockSource::RtcClkDiv16)
                .async_prescaler(31)
                .sync_prescaler(1023),
            _ => RtcConfig::default()
                .clock_config(RtcClockSource::LSI)
                .wakeup_clock_config(RtcWakeupClockSource::RtcClkDiv16)
                .async_prescaler(31)
                .sync_prescaler(999),
        };

        let mut rtc = Rtc::rtc(rtc, apb1r1, bdcr, pwrcr1, rtc_config);
        rtc.listen(exti, Event::WakeupTimer);

        if rtc.read_backup_register(0) != Some(RTC_INIT_MARKER) {
            rtc.set_date_time(
                Date::new(4.day(), 1.date(), 1.month(), 2026.year()),
                Time::new(
                    Hour::hours(0),
                    Minute::minutes(0),
                    Second::secs(0),
                    Micros::micros(0),
                    false,
                ),
            );
            rtc.write_backup_register(0, RTC_INIT_MARKER);
        }

        (
            Self {
                rtc,
                rtc_clock_source: source,
                wakeup_ticks: None,
            },
            source,
        )
    }

    pub fn set_alarm_period_ms(&mut self, period_ms: u32) {
        let period_ms = period_ms.max(1);
        let rtc_clock_hz = self.rtc_clock_hz();
        let wakeup_clock_hz = rtc_clock_hz / 16;

        let ticks = ((period_ms as u64 * wakeup_clock_hz as u64) + 999) / 1000;
        let ticks = ticks.clamp(1, 65_536) as u32;

        self.wakeup_ticks = Some(ticks);
        self.restart_wakeup_timer();
    }

    pub fn current_time(&self) -> CurrentTime {
        let (date, time) = self.rtc.get_date_time();
        CurrentTime {
            year: date.year,
            month: date.month,
            day_of_month: date.date,
            day_of_week: date.day,
            hours: time.hours,
            minutes: time.minutes,
            seconds: time.seconds,
            milliseconds: time.micros / 1_000,
        }
    }

    pub fn set_time(&mut self, time: CurrentTime) {
        use stm32l4xx_hal::datetime::{DateInMonth, Day, Hour, Minute, Month, Second, Time, Year};
        self.rtc.set_date_time(
            Date::new(
                Day(time.day_of_week),
                DateInMonth(time.day_of_month),
                Month(time.month),
                Year(time.year),
            ),
            Time::new(
                Hour::hours(time.hours),
                Minute::minutes(time.minutes),
                Second::secs(time.seconds),
                Micros::micros(time.milliseconds * 1_000),
                false,
            ),
        );
    }

    pub fn handle_alarm_interrupt(&mut self) -> bool {
        if !self.rtc.check_interrupt(Event::WakeupTimer, true) {
            return false;
        }

        self.restart_wakeup_timer();
        true
    }

    fn restart_wakeup_timer(&mut self) {
        let Some(ticks) = self.wakeup_ticks else {
            return;
        };

        self.rtc.wakeup_timer().start(ticks);
    }

    fn rtc_clock_hz(&self) -> u32 {
        match self.rtc_clock_source {
            RtcClockSource::LSE => 32_768,
            RtcClockSource::LSI => 32_000,
            _ => 32_000,
        }
    }
}

fn select_rtc_clock_source() -> RtcClockSource {
    let rcc = unsafe { &*stm32l4xx_hal::pac::RCC::ptr() };
    let pwr = unsafe { &*stm32l4xx_hal::pac::PWR::ptr() };

    pwr.cr1.modify(|_, w| w.dbp().set_bit());
    while pwr.cr1.read().dbp().bit_is_clear() {}

    rcc.bdcr
        .modify(|_, w| w.lsebyp().clear_bit().lseon().set_bit());

    for _ in 0..LSE_STARTUP_TIMEOUT_CYCLES {
        if rcc.bdcr.read().lserdy().bit_is_set() {
            return RtcClockSource::LSE;
        }
    }

    rcc.bdcr.modify(|_, w| w.lseon().clear_bit());
    rcc.csr.modify(|_, w| w.lsion().set_bit());

    for _ in 0..LSI_STARTUP_TIMEOUT_CYCLES {
        if rcc.csr.read().lsirdy().bit_is_set() {
            break;
        }
    }

    RtcClockSource::LSI
}

impl RtcCalibrationOutput for RtcService {
    fn enable_calibration_output(
        &mut self,
        pin: impl Into<RtcCalibrationOutputPin>,
        _frequency: Hertz,
    ) -> Result<(), ()> {
        let _pin = pin.into();

        Ok(())
    }
}

//-----------------------------------------------------------------------------

macro_rules! impl_rtc_calibration_output_pin {
    ($($pin_type:ty),+ $(,)?) => {
        $(
            impl Into<RtcCalibrationOutputPin> for $pin_type {
                fn into(self) -> RtcCalibrationOutputPin {
                    RtcCalibrationOutputPin {}
                }
            }
        )+
    };
}

impl_rtc_calibration_output_pin!(PC13<Alternate<PushPull, 0>>, PB2<Alternate<PushPull, 0>>);
