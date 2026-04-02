use serde::de;
use stm32l4xx_hal::{
    datetime::{Date, Time, U32Ext},
    pac::{self, PWR, RCC},
    pwr,
    rcc::{APB1R1, BDCR},
    rtc::{Alarm, Event, Rtc, RtcClockSource, RtcConfig},
};

const RTC_INIT_MARKER: u32 = 0xA5A5_5A5A;
const LSE_STARTUP_TIMEOUT_CYCLES: usize = 200_000;
const LSI_STARTUP_TIMEOUT_CYCLES: usize = 200_000;

#[derive(Clone, Copy, Debug)]
pub struct CurrentTime {
    pub year: u32,
    pub month: u32,
    pub day_of_month: u32,
    pub day_of_week: u32,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub milliseconds: u32,
}

impl defmt::Format for CurrentTime {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            self.year,
            self.month,
            self.day_of_month,
            self.hours,
            self.minutes,
            self.seconds,
            self.milliseconds
        );
    }
}

pub struct RtcService {
    rtc: Rtc,
    alarm_period_ms: Option<u32>,
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
                .async_prescaler(31)
                .sync_prescaler(1023),
            _ => RtcConfig::default()
                .clock_config(RtcClockSource::LSI)
                .async_prescaler(31)
                .sync_prescaler(999),
        };

        let mut rtc = Rtc::rtc(rtc, apb1r1, bdcr, pwrcr1, rtc_config);
        rtc.listen(exti, Event::AlarmA);

        if rtc.read_backup_register(0) != Some(RTC_INIT_MARKER) {
            rtc.set_date_time(
                Date::new(4.day(), 1.date(), 1.month(), 2026.year()),
                Time::new(0.hours(), 0.minutes(), 0.seconds(), 0.micros(), false),
            );
            rtc.write_backup_register(0, RTC_INIT_MARKER);
        }

        (
            Self {
                rtc,
                alarm_period_ms: None,
            },
            source,
        )
    }

    pub fn set_alarm_period_ms(&mut self, period_ms: u32) {
        self.alarm_period_ms = Some(period_ms.max(1));
        self.schedule_next_alarm();
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

    pub fn handle_alarm_interrupt(&mut self) -> bool {
        if !self.rtc.check_interrupt(Event::AlarmA, true) {
            return false;
        }

        self.schedule_next_alarm();
        true
    }

    fn schedule_next_alarm(&mut self) {
        let Some(period_ms) = self.alarm_period_ms else {
            return;
        };

        let (date, time) = self.rtc.get_date_time();
        let add_seconds = ((period_ms - 1) / 1_000) + 1;
        let (next_date, next_time) = add_seconds_to_date_time(date, time, add_seconds);
        self.rtc.set_alarm(Alarm::AlarmA, next_date, next_time);
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

fn add_seconds_to_date_time(mut date: Date, time: Time, seconds_to_add: u32) -> (Date, Time) {
    let day_seconds = 24 * 60 * 60;
    let total_seconds = time.hours * 3_600 + time.minutes * 60 + time.seconds + seconds_to_add;
    let mut days_to_add = total_seconds / day_seconds;
    let seconds_of_day = total_seconds % day_seconds;

    let hours = seconds_of_day / 3_600;
    let minutes = (seconds_of_day % 3_600) / 60;
    let seconds = seconds_of_day % 60;

    while days_to_add > 0 {
        let dim = days_in_month(date.year, date.month);
        if date.date < dim {
            date.date += 1;
        } else {
            date.date = 1;
            if date.month < 12 {
                date.month += 1;
            } else {
                date.month = 1;
                date.year += 1;
            }
        }
        date.day = if date.day >= 7 { 1 } else { date.day + 1 };
        days_to_add -= 1;
    }

    (
        Date::new(
            date.day.day(),
            date.date.date(),
            date.month.month(),
            date.year.year(),
        ),
        Time::new(
            hours.hours(),
            minutes.minutes(),
            seconds.seconds(),
            0.micros(),
            false,
        ),
    )
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
