use qspi_stm32lx3::{
    qspi::{ClkPin, IO0Pin, IO1Pin, IO2Pin, IO3Pin, NCSPin},
    stm32l4x3::QUADSPI,
};
use stm32_usb_self_writer::{
    config, qspi_storage::QSPIStorage, sensors::analog::AnalogSensor, settings,
};
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
use usb_device::bus::UsbBusAllocator;
use usbd_serial::CdcAcmClass;

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

pub fn init_usb<'a, USB: stm32_usbd::UsbPeripheral>(
    is_enabled: bool,
    periph: USB,
    bus: &'a mut Option<UsbBusAllocator<stm32_usbd::UsbBus<USB>>>,
    vid_pid: usb_device::device::UsbVidPid,
) -> (
    usb_device::device::UsbDevice<'a, stm32_usbd::UsbBus<USB>>,
    usbd_scsi::Scsi<'a, stm32_usbd::UsbBus<USB>, stm32_usb_self_writer::vfs::EMfatStorage>,
    CdcAcmClass<'a, stm32_usbd::UsbBus<USB>>,
) {
    if !is_enabled {
        defmt::info!("\tUSB not enabled, skipping USB initialization");
        return unsafe {
            (
                #[allow(invalid_value)]
                core::mem::MaybeUninit::zeroed().assume_init(),
                #[allow(invalid_value)]
                core::mem::MaybeUninit::zeroed().assume_init(),
                #[allow(invalid_value)]
                core::mem::MaybeUninit::zeroed().assume_init(),
            )
        };
    }

    defmt::info!("Creating usb low-level driver: PA11, PA12, AF10");

    let bus: &'a mut UsbBusAllocator<stm32_usbd::UsbBus<USB>> =
        bus.get_or_insert(stm32_usbd::UsbBus::new(periph));

    defmt::info!("Allocating SCSI device");
    let scsi = usbd_scsi::Scsi::new(
        bus,
        config::BULK_MAX_PACKET_SIZE as u16, // для устройств full speed: max_packet_size 8, 16, 32 or 64
        stm32_usb_self_writer::vfs::EMfatStorage::new(my_proc_macro::c_str!("LOGGER")),
        "SCTB", // <= max 8 больших букв
        "SelfWriter",
        "L433",
    );

    defmt::info!("Allocating ACM device");
    let serial = usbd_serial::CdcAcmClass::new(bus, config::BULK_MAX_PACKET_SIZE as u16);

    defmt::info!("Building usb device: vid={} pid={}", &vid_pid.0, &vid_pid.1);
    let usb_dev: usb_device::prelude::UsbDevice<'a, stm32_usbd::UsbBus<USB>> =
        usb_device::device::UsbDeviceBuilder::new(bus, vid_pid)
            .manufacturer("SCTB ELPA")
            .product("Pressure self-registrator")
            .serial_number(stm32_device_signature::device_id_hex())
            .composite_with_iads()
            .build();

    defmt::info!("USB ready!");

    (usb_dev, scsi, serial)
}

pub fn init_storage<M, R, CLK, NCS1, IO0_1, IO1_1, IO2_1, IO3_1, NCS2, IO0_2, IO1_2, IO2_2, IO3_2>(
    qspi: QUADSPI,
    mut flash_reset_pin: R,
    clk_pin: CLK,
    pins_ch1: (NCS1, IO0_1, IO1_1, IO2_1, IO3_1),
    pins_ch2: (NCS2, IO0_2, IO1_2, IO2_2, IO3_2),
    rcc: &mut rcc::Rcc,
    clocks: &Clocks,
) -> QSPIStorage
where
    M: rtic_monotonics::Monotonic<Duration = config::Duration, Instant = config::Instant> + 'static,
    R: embedded_hal::digital::v2::OutputPin + 'static,
    CLK: ClkPin<QUADSPI> + 'static,
    NCS1: NCSPin<QUADSPI> + 'static,
    IO0_1: IO0Pin<QUADSPI> + 'static,
    IO1_1: IO1Pin<QUADSPI> + 'static,
    IO2_1: IO2Pin<QUADSPI> + 'static,
    IO3_1: IO3Pin<QUADSPI> + 'static,
    NCS2: NCSPin<QUADSPI> + 'static,
    IO0_2: IO0Pin<QUADSPI> + 'static,
    IO1_2: IO1Pin<QUADSPI> + 'static,
    IO2_2: IO2Pin<QUADSPI> + 'static,
    IO3_2: IO3Pin<QUADSPI> + 'static,
{
    flash_reset_pin.set_low().ok();
    cortex_m::asm::delay(clocks.sysclk().0 / 100); // ~10ms delay
    flash_reset_pin.set_high().ok();

    let mut qspi_ch1 = qspi_stm32lx3::qspi::Qspi::new_bank1(
        qspi,
        (
            clk_pin, pins_ch1.0, pins_ch1.1, pins_ch1.2, pins_ch1.3, pins_ch1.4,
        ),
        unsafe { core::mem::transmute(&mut rcc.ahb3) },
        qspi_stm32lx3::qspi::QspiConfig::default(),
    );

    let id1 = stm32_usb_self_writer::qspi_storage::probe(&mut qspi_ch1, clocks.sysclk());
    match &id1 {
        Ok(id) => {
            defmt::info!("QSPI flash bank 1 detected, {}", defmt::Debug2Format(&id));
        }
        Err(e) => {
            defmt::warn!(
                "QSPI flash bank 1 not detected: {}",
                defmt::Debug2Format(&e)
            );
        }
    }

    let (qspi, (clk, ncs1, io0_1, io1_1, io2_1, io3_1)) = qspi_ch1.destroy();
    let (ncs2_ch, io0_2_ch, io1_2_ch, io2_2_ch, io3_2_ch) = pins_ch2;

    // Проверить id1; если ошибка - создаем ch2 только для проверки
    if id1.is_ok() {
        // Bank1 работает - попытаться создать ch2 для dual-режима

        // Для проверки ch2: используем QUADSPI в bank2 режиме
        let mut qspi_ch2_probe = qspi_stm32lx3::qspi::Qspi::new_bank2(
            qspi,
            (clk, ncs2_ch, io0_2_ch, io1_2_ch, io2_2_ch, io3_2_ch),
            unsafe { core::mem::transmute(&mut rcc.ahb3) },
            qspi_stm32lx3::qspi::QspiConfig::default(),
        );

        let id2 = stm32_usb_self_writer::qspi_storage::probe(&mut qspi_ch2_probe, clocks.sysclk());
        match &id2 {
            Ok(id) => {
                defmt::info!("QSPI flash bank 2 detected, {}", defmt::Debug2Format(&id));
            }
            Err(e) => {
                defmt::warn!(
                    "QSPI flash bank 2 not detected: {}",
                    defmt::Debug2Format(&e)
                );
            }
        }

        // Принять решение на основе результатов проб
        match (id1, id2) {
            (Ok(id_ch1), Ok(id_ch2)) => {
                // Оба канала работают - проверить совпадение IDs
                if id_ch1 == id_ch2 {
                    defmt::info!(
                        "Both banks detected with matching IDs. Creating dual-channel driver."
                    );

                    // Получить пины обратно из ch2_probe
                    let (qspi_dual, (clk_ret, ncs2, io0_2, io1_2, io2_2, io3_2)) =
                        qspi_ch2_probe.destroy();

                    // Создать dual-режим драйвер
                    let pins_dual = (
                        clk_ret, ncs1, io0_1, io1_1, io2_1, io3_1, ncs2, io0_2, io1_2, io2_2, io3_2,
                    );

                    let qspi_dual = qspi_stm32lx3::qspi::Qspi::new_dual(
                        qspi_dual,
                        pins_dual,
                        unsafe { core::mem::transmute(&mut rcc.ahb3) },
                        qspi_stm32lx3::qspi::QspiConfig::default(),
                    );

                    QSPIStorage::new::<_, M>(qspi_dual, id_ch1, true, clocks.sysclk())
                        .expect("Failed to initialize dual-channel QSPI driver")
                } else {
                    defmt::panic!(
                        "JDEC ID mismatch! Bank1: {}, Bank2: {} - possible PCB/assembly issue",
                        defmt::Debug2Format(&id_ch1),
                        defmt::Debug2Format(&id_ch2)
                    );
                }
            }
            (Ok(id_ch1), Err(_)) => {
                defmt::warn!("Bank 2 detection failed. Using single-channel Bank 1 driver.");

                // Освободить пины ch2_probe
                let (qspi, (clk_pin, _, _, _, _, _)) = qspi_ch2_probe.destroy();

                let qspi_ch1 = qspi_stm32lx3::qspi::Qspi::new_bank1(
                    qspi,
                    (clk_pin, ncs1, io0_1, io1_1, io2_1, io3_1),
                    unsafe { core::mem::transmute(&mut rcc.ahb3) },
                    qspi_stm32lx3::qspi::QspiConfig::default(),
                );

                QSPIStorage::new::<_, M>(qspi_ch1, id_ch1, false, clocks.sysclk())
                    .expect("Failed to initialize dual-channel QSPI driver")
            }
            // Остальные комбинации невозможны, т.к. id1 = Ok по условию выше
            _ => unreachable!(),
        }
    } else {
        // Bank1 не работает - попытаться ch2
        let mut qspi_ch2 = qspi_stm32lx3::qspi::Qspi::new_bank2(
            qspi,
            (clk, ncs2_ch, io0_2_ch, io1_2_ch, io2_2_ch, io3_2_ch),
            unsafe { core::mem::transmute(&mut rcc.ahb3) },
            qspi_stm32lx3::qspi::QspiConfig::default(),
        );

        let id2 = stm32_usb_self_writer::qspi_storage::probe(&mut qspi_ch2, clocks.sysclk());
        match id2 {
            Ok(id) => {
                defmt::info!("QSPI flash bank 2 detected, {}", defmt::Debug2Format(&id));

                QSPIStorage::new::<_, M>(qspi_ch2, id, false, clocks.sysclk())
                    .expect("Failed to initialize dual-channel QSPI driver")
            }
            Err(_) => {
                defmt::panic!("No QSPI flash detected on any bank!");
            }
        }
    }
}
