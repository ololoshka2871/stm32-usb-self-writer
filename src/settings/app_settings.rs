use num_derive::FromPrimitive;
use serde::Serialize;

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct P16Coeffs {
    pub fp0: f32,
    pub ft0: f32,
    pub a: [f32; /*crate::protobuf::P_COEFFS_COUNT*/ 16],
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct T5Coeffs {
    pub f0: f32,
    pub t0: f32,
    pub c: [f32; /*crate::protobuf::T_COEFFS_COUNT*/ 5],
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorkRange {
    pub minimum: f32,
    pub maximum: f32,
    pub absolute_maximum: f32,
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CalibrationDate {
    pub day: u32,
    pub month: u32,
    pub year: u32,
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WriteConfig {
    pub base_interval_ms: u32,
    pub p_write_devider: u32,
    pub t_write_devider: u32,
}

#[repr(packed(1))]
#[derive(Debug, Copy, Clone, Serialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct Monitoring {
    pub ovarpress: bool,
    pub ovarheat: bool,
    pub cpu_ovarheat: bool,
    pub over_power: bool,
}

#[derive(Debug, Clone, Copy, Serialize, FromPrimitive)]
pub enum PressureMeassureUnits {
    InvalidZero = 0,

    // Паскали
    Pa = 0x00220000,

    // Бар
    Bar = 0x004E0000,

    // Ат (кгс/см2)
    At = 0x00A10000,

    // мм водного столба
    MmH20 = 0x00A20000,

    // м. ртутного столба
    MHg = 0x00A30000,

    // Атм
    Atm = 0x00A40000,

    // Фунт на квадратный дюйм
    PSI = 0x00AB0000,
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AppSettings {
    pub serial: u32,

    pub fref: u32,

    pub p_coefficients: P16Coeffs,
    pub t_coefficients: T5Coeffs,

    pub p_work_range: WorkRange,
    pub t_work_range: WorkRange,
    pub t_cpu_work_range: WorkRange,
    pub vbat_work_range: WorkRange,

    pub p_zero_correction: f32,
    pub t_zero_correction: f32,

    pub calibration_date: CalibrationDate,

    pub write_config: WriteConfig,

    pub start_delay: u32,

    pub pressure_meassure_units: PressureMeassureUnits,

    #[serde(skip_serializing)]
    pub password: [u8; /*crate::protobuf::PASSWORD_SIZE*/ 10],

    pub monitoring: Monitoring,
}

#[derive(Debug, Copy, Clone)]
pub struct NonStoreSettings {
    pub current_password: [u8; 10],
}

impl Monitoring {
    pub fn is_set(&self) -> bool {
        self.ovarpress | self.ovarheat | self.cpu_ovarheat | self.over_power
    }
}
