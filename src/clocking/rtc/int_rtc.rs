use num_traits::float::FloatCore;

use stm32l4xx_hal::{
    datetime::{Date, Hour, Micros, Minute, Second, Time, U32Ext},
    gpio::{Alternate, Analog, PB2, PC13, PushPull},
    hal::timer::CountDown,
    pac::{self},
    pwr,
    rcc::{APB1R1, BDCR},
    rtc::{Event, Rtc, RtcClockSource, RtcConfig, RtcWakeupClockSource},
    time::Hertz,
};

use super::{
    CurrentTime, RtcCalibrationOutput, RtcCalibrationOutputPin, RtcTrimming, RtcTrimmingError,
};

const RTC_INIT_MARKER: u32 = 0xA5A5_5A5A;
const LSE_STARTUP_TIMEOUT_CYCLES: usize = 2000_000;
const LSI_STARTUP_TIMEOUT_CYCLES: usize = 2000_000;

const TRIMMING_ACCURACY_PPM: f32 = 0.954;

pub struct RtcService {
    rtc: Rtc,
    rtc_clock_source: RtcClockSource,
    wakeup_ticks: Option<u32>,
    last_calibration_error_ppm: f32,
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
                // p. 35.3.4 Clock and prescalers
                .async_prescaler(63) // <= 127
                .sync_prescaler(511), // to count 2*miliseconds
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
                last_calibration_error_ppm: 0.0,
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

    fn with_unlocked(&mut self, f: impl FnOnce(&stm32l4xx_hal::pac::rtc::RegisterBlock)) {
        // RTC register writes are protected by a write protection mechanism that requires
        // unlocking with specific keys. This function handles the unlocking and relocking.
        let rtc = unsafe { &*pac::RTC::ptr() };
        rtc.wpr.write(|w| unsafe { w.key().bits(0xCA) });
        rtc.wpr.write(|w| unsafe { w.key().bits(0x53) });

        f(rtc);

        rtc.wpr.write(|w| unsafe { w.key().bits(0xFF) });
    }
}

fn select_rtc_clock_source() -> RtcClockSource {
    let rcc = unsafe { &*stm32l4xx_hal::pac::RCC::ptr() };
    let pwr = unsafe { &*stm32l4xx_hal::pac::PWR::ptr() };

    pwr.cr1.modify(|_, w| w.dbp().set_bit());
    while pwr.cr1.read().dbp().bit_is_clear() {}

    // Некоторые кварцы не запускаются при lsedrv < 0b10, или выдают нестабильную/неправильную частоту
    rcc.bdcr
        .modify(|_, w| unsafe { w.lsebyp().clear_bit().lseon().set_bit().lsedrv().bits(0b00) });

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

impl RtcTrimming for RtcService {
    fn set_calibration(&mut self, calibration_ppm: f32) -> Result<(), RtcTrimmingError> {
        let magnitude = (calibration_ppm + self.last_calibration_error_ppm) / TRIMMING_ACCURACY_PPM;
        let magnitude_clamped = magnitude.clamp(-511.0, 512.0).round();
        self.last_calibration_error_ppm = magnitude - magnitude_clamped;
        let (calp, calm) = if magnitude_clamped >= 0.0 {
            (true, magnitude_clamped as u16)
        } else {
            (false, (-magnitude_clamped) as u16)
        };

        self.with_unlocked(|rtc| {
            rtc.calr.modify(|_, w| unsafe {
                w.calp()
                    .bit(calp)
                    .calm()
                    .bits(calm)
                    .calw16()
                    .clear_bit()
                    .calw8()
                    .clear_bit()
            });
        });

        defmt::debug!(
            "RTC calibration: {} ppm ({}, error {} ppm)",
            calibration_ppm,
            calm,
            self.last_calibration_error_ppm
        );

        Ok(())
    }

    fn get_calibration(&self) -> f32 {
        let rtc = unsafe { &*pac::RTC::ptr() };
        let calib = rtc.calr.read();
        let sign = if calib.calp().bit_is_set() { 1.0 } else { -1.0 };
        let magnitude = calib.calm().bits() as f32;

        sign * magnitude * TRIMMING_ACCURACY_PPM + self.last_calibration_error_ppm
    }
}

impl RtcCalibrationOutput for RtcService {
    // FCAL = FRTCCLK x [1 + (CALP x 512 - CALM) / (220 + CALM - CALP x 512)]
    // CALP - 1 бит - знак (0 - отрицательная коррекция, 1 - положительная коррекция)
    // CALM - 9 бит - величина коррекции (0..=511) * 0.954 ppm
    // Того без учета коррекции частота калибровоная 32768 / 220 = 148,9(45) Hz
    fn enable_calibration_output(
        &mut self,
        pin: impl Into<RtcCalibrationOutputPin>,
        frequency: Hertz,
    ) -> Result<(), ()> {
        let pin = pin.into();

        let output_1hz = match frequency.to_Hz() {
            512 => false,
            2 => true,
            _ => return Err(()),
        };

        self.with_unlocked(|rtc| {
            // Configure the RTC output remap based on the pin used
            rtc.or.modify(|_, w| w.rtc_out_rmp().bit(pin.is_remap()));

            rtc.cr.modify(|_, w| unsafe {
                w.osel().bits(0b00).cosel().bit(output_1hz).coe().set_bit()
            });
        });

        Ok(())
    }
}

//-----------------------------------------------------------------------------

macro_rules! impl_rtc_calibration_output_pin {
    ($(($pin_type:ty, $remap:literal)),+ $(,)?) => {
        $(
            impl Into<RtcCalibrationOutputPin> for $pin_type {
                fn into(self) -> RtcCalibrationOutputPin {
                    RtcCalibrationOutputPin::new($remap)
                }
            }
        )+
    };
}

impl_rtc_calibration_output_pin!(
    (PC13<Analog>, false),
    (PB2<Alternate<PushPull, 0>>, true)
);
