mod app_settings;
mod flash_rw_polcy;
mod store_async;

pub use app_settings::*;
use flash_settings_rs::SettingsManager;

pub use flash_rw_polcy::{FlasRWPolcy, Placeholder};
pub use my_proc_macro::{build_day, build_month, build_year};

use crate::support::crc::ZlibCompantCrc32;

pub static MAX_MT: u32 = 5000;
pub static MIN_MT: u32 = 20;

static DEFAULT_SETTINGS: AppSettings = AppSettings {
    serial: 0,

    fref: crate::config::XTAL_FREQ,

    p_coefficients: app_settings::P16Coeffs {
        fp0: 0.0,
        ft0: 0.0,
        a: [
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
    },
    t_coefficients: app_settings::T5Coeffs {
        f0: 0.0,
        t0: 0.0,
        c: [1.0, 0.0, 0.0, 0.0, 0.0],
    },

    p_work_range: app_settings::WorkRange {
        minimum: 0.0,
        maximum: 100.0,
        absolute_maximum: f32::NAN,
    },

    t_work_range: app_settings::WorkRange {
        minimum: -50.0,
        maximum: 120.0,
        absolute_maximum: f32::NAN,
    },

    t_cpu_work_range: app_settings::WorkRange {
        minimum: -50.0,
        maximum: 120.0,
        absolute_maximum: f32::NAN,
    },

    vbat_work_range: app_settings::WorkRange {
        minimum: 2.2, //2.05 for TPS6223xx
        maximum: 5.5,
        absolute_maximum: 6.0, // TPS6223xx
    },

    calibration_date: app_settings::CalibrationDate {
        day: build_day!(),
        month: build_month!(),
        year: build_year!(),
    },

    p_zero_correction: 0.0,
    t_zero_correction: 0.0,

    write_config: app_settings::WriteConfig {
        base_interval_ms: 20,
        p_write_devider: 1,
        t_write_devider: 1,
    },

    start_delay: 0,

    pressure_meassure_units: app_settings::PressureMeassureUnits::Bar,

    password: *b"_PASSWORD_",

    monitoring: app_settings::Monitoring {
        ovarpress: false,
        ovarheat: false,
        cpu_ovarheat: false,
        over_power: false,
    },
};

pub type SettingsManagerType = SettingsManager<AppSettings, NonStoreSettings>;

#[unsafe(link_section = ".settings.app")]
static SETTINGS_PLACEHOLDER: Placeholder<AppSettings> =
    unsafe { core::mem::transmute([0u8; core::mem::size_of::<Placeholder<AppSettings>>()]) };

pub fn init<CRC: ZlibCompantCrc32>(
    flash: stm32l4xx_hal::flash::Parts,
    crc: CRC,
) -> (SettingsManagerType, FlasRWPolcy<AppSettings, CRC>) {
    defmt::trace!("Init settings");
    let mut policy = FlasRWPolcy::create(&SETTINGS_PLACEHOLDER, flash, crc);
    (
        SettingsManagerType::new(
            &DEFAULT_SETTINGS,
            NonStoreSettings {
                current_password: [0u8; 10],
            },
            &mut policy,
        ),
        policy,
    )
}
