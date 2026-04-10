use stm32_usb_self_writer::{config, sensors::analog::AnalogSensor, settings};
use stm32l4xx_hal::{
    adc,
    crc::CrcExt,
    flash,
    gpio::{Analog, gpioa::PA1},
    pac,
    prelude::*,
    pwr,
    rcc::{self, Clocks},
    time::Hertz,
};

use stm32_usb_self_writer::{clocking::ClockConfigProvider, support::crc::STM32L4Crc32};

use crate::types;

pub fn init_clocks(
    fast_mode: bool,
    flash: &mut flash::Parts,
    rcc: &mut rcc::Rcc,
    pwr: &mut pwr::Pwr,
) -> (Clocks, Hertz, bool) {
    let res = if fast_mode {
        defmt::info!("\tUSB connected, starting in high performance mode");
        (
            types::HighPerformanceClockProvider::configure_clocks(flash, rcc, pwr),
            types::HighPerformanceClockProvider::master_counter_frequency(),
            true,
        )
    } else {
        defmt::info!("\tUSB not connected, starting in recorder mode");
        (
            types::RecorderClockProvider::configure_clocks(flash, rcc, pwr),
            types::RecorderClockProvider::master_counter_frequency(),
            false,
        )
    };
    defmt::info!("\tClocks: {}", defmt::Debug2Format(&res.0));

    res
}

pub fn init_settings(
    flash: flash::Parts,
    crc: pac::CRC,
    rcc: &mut rcc::Rcc,
    high_perf_mode: bool,
) -> (
    settings::SettingsManagerType,
    settings::FlasRWPolcy<settings::AppSettings, STM32L4Crc32>,
    config::Duration,
    config::Duration,
    settings::WriteConfig,
) {
    let (mut settings, flash_policy) =
        settings::init(flash, STM32L4Crc32::new(crc.constrain(&mut rcc.ahb1)));
    let config = settings.ref_mut().0;
    let write_config = config.write_config.clone();

    let base_period = config::Duration::millis(write_config.base_interval_ms);
    let start_delay = config::Duration::secs(if high_perf_mode {
        0
    } else {
        config.start_delay
    });

    defmt::info!(
        "\tSettings loaded, base period: {} ms, start delay: {} s",
        write_config.base_interval_ms,
        start_delay.to_secs()
    );

    (
        settings,
        flash_policy,
        base_period,
        start_delay,
        write_config,
    )
}

pub fn init_rtc_service(
    base_period: config::Duration,
    rtc: pac::RTC,
    exti: &mut pac::EXTI,
    apb1r1: &mut rcc::APB1R1,
    bdcr: &mut rcc::BDCR,
    pwrcr1: &mut pwr::CR1,
) -> stm32_usb_self_writer::clocking::rtc::RtcService {
    let (mut rtc, rtc_clock_source) =
        stm32_usb_self_writer::clocking::rtc::RtcService::init(rtc, exti, apb1r1, bdcr, pwrcr1);

    rtc.set_alarm_period_ms(base_period.to_millis() as u32);
    defmt::info!(
        "\tRTC initialized, source: {}",
        defmt::Debug2Format(&rtc_clock_source)
    );

    rtc
}

pub fn init_analog_sensors(
    clocks: &Clocks,
    adc: pac::ADC1,
    common: pac::ADC_COMMON,
    ahb: &mut rcc::AHB2,
    ccipr: &mut rcc::CCIPR,
    vbat_pin: PA1<Analog>,
) -> AnalogSensor<PA1<Analog>> {
    let res = {
        let mut delay = stm32_usb_self_writer::NOPDelay {
            sys_clk: clocks.sysclk(),
        };

        let adc = adc::ADC::new(adc, common, ahb, ccipr, &mut delay);

        AnalogSensor::new(adc, vbat_pin, &mut delay)
    };
    defmt::info!("\tAnalog sensor");

    res
}

pub fn init_master_timer(
    tim6: pac::TIM6,
    clocks: Clocks,
    apb1r1: &mut rcc::APB1R1,
) -> types::MasterCounter {
    let mut master_timer = types::MasterCounter::new(stm32l4xx_hal::timer::Timer::tim6(
        tim6,
        1.hz(),
        clocks,
        apb1r1,
    ));

    master_timer.listen();
    defmt::info!("\tMaster timer");

    master_timer
}
