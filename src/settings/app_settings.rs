#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use corelogic::app_settings::WriteConfig;
use serde::Serialize;

pub use corelogic::{
    app_settings::{
        CalibrationDate, Monitoring, P16Coeffs, PressureMeassureUnits, T5Coeffs, WorkRange,
    },
    protobuf::PASSWORD_SIZE,
};

#[derive(Debug, Copy, Clone, Serialize)]
pub struct AppSettings {
    pub Serial: u32,
    pub PMesureTime_ms: u32,
    pub T1MesureTime_ms: u32,
    pub T2MesureTime_ms: u32,

    pub Fref: u32,

    pub P_enabled: bool,
    pub T1_enabled: bool,
    pub T2_enabled: bool,
    pub TCPUEnabled: bool,
    pub VBatEnabled: bool,

    pub P_Coefficients: P16Coeffs,
    pub T1_Coefficients: T5Coeffs,
    pub T2_Coefficients: T5Coeffs,

    pub PWorkRange: WorkRange,
    pub TWorkRange: WorkRange,
    pub TCPUWorkRange: WorkRange,
    pub VbatWorkRange: WorkRange,

    pub PZeroCorrection: f32,
    pub TZeroCorrection: f32,

    pub calibration_date: CalibrationDate,

    pub writeConfig: WriteConfig,

    pub startDelay: u32,

    pub pressureMeassureUnits: PressureMeassureUnits,

    #[serde(skip_serializing)]
    pub password: [u8; PASSWORD_SIZE],

    pub monitoring: Monitoring,
}