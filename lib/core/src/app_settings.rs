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

impl Monitoring {
    pub fn is_set(&self) -> bool {
        self.Ovarpress | self.Ovarheat | self.CPUOvarheat | self.OverPower
    }
}

#[derive(Debug, Clone, Copy, Serialize, num_derive::FromPrimitive)]
pub enum PressureMeassureUnits {
    INVALID_ZERO = 0,

    /// Паскали
    Pa = 0x00220000,

    /// Бар
    Bar = 0x004E0000,

    /// Ат (кгс/см2)
    At = 0x00A10000,

    /// мм водного столба
    mmH20 = 0x00A20000,

    /// м. ртутного столба
    mHg = 0x00A30000,

    /// Атм
    Atm = 0x00A40000,

    /// Фунт на квадратный дюйм
    PSI = 0x00AB0000,
}

#[derive(Debug, Copy, Clone)]
pub struct NonStoreSettings {
    pub current_password: [u8; PASSWORD_SIZE],
}
