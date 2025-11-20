#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use serde::Serialize;
use super::protobuf::{P_COEFFS_COUNT, T_COEFFS_COUNT, PASSWORD_SIZE};

#[derive(Debug, Copy, Clone, Serialize)]
pub struct P16Coeffs {
    pub Fp0: f32,
    pub Ft0: f32,
    pub A: [f32; P_COEFFS_COUNT],
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct T5Coeffs {
    pub F0: f32,
    pub T0: f32,
    pub C: [f32; T_COEFFS_COUNT],
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct WorkRange {
    pub minimum: f32,
    pub maximum: f32,
    pub absolute_maximum: f32,
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct CalibrationDate {
    pub Day: u32,
    pub Month: u32,
    pub Year: u32,
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct WriteConfig {
    pub BaseInterval_ms: u32,
    pub PWriteDevider: u32,
    pub TWriteDevider: u32,
}

#[repr(packed(1))]
#[derive(Debug, Copy, Clone, Serialize, Default)]
pub struct Monitoring {
    pub Ovarpress: bool,
    pub Ovarheat: bool,
    pub CPUOvarheat: bool,
    pub OverPower: bool,
}

#[derive(Debug, Clone, Copy, Serialize, num_derive::FromPrimitive)]
pub enum PressureMeassureUnits {
    INVALID_ZERO = 0,
    Pa = 0x00220000,
    Bar = 0x004E0000,
    At = 0x00A10000,
    mmH20 = 0x00A20000,
    mHg = 0x00A30000,
    Atm = 0x00A40000,
    PSI = 0x00AB0000,
}

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

#[derive(Debug, Copy, Clone)]
pub struct NonStoreSettings {
    pub current_password: [u8; PASSWORD_SIZE],
}

impl Monitoring {
    pub fn is_set(&self) -> bool {
        self.Ovarpress | self.Ovarheat | self.CPUOvarheat | self.OverPower
    }
}
