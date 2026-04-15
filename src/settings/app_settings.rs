use num_derive::FromPrimitive;
use serde::Serialize;

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct P16Coeffs {
    pub fp0: f32,
    pub ft0: f32,
    pub a: [f32; crate::protobuf::P_COEFFS_COUNT],
}

impl P16Coeffs {
    pub fn calc(&self, f: Option<f64>, t: Option<f64>) -> f64 {
        match (f, t) {
            (Some(f), t) => {
                let presf_minus_fp0 = f - self.fp0 as f64;
                let ft_minus_ft0 = if let Some(t) = t {
                    t - self.ft0 as f64
                } else {
                    0.0
                };

                let a = &self.a;

                let k0 = a[0] as f64
                    + ft_minus_ft0
                        * (a[1] as f64
                            + ft_minus_ft0 * (a[2] as f64 + ft_minus_ft0 * a[12] as f64));
                let k1 = a[3] as f64
                    + ft_minus_ft0
                        * (a[5] as f64
                            + ft_minus_ft0 * (a[7] as f64 + ft_minus_ft0 * a[13] as f64));
                let k2 = a[4] as f64
                    + ft_minus_ft0
                        * (a[6] as f64
                            + ft_minus_ft0 * (a[8] as f64 + ft_minus_ft0 * a[14] as f64));
                let k3 = a[9] as f64
                    + ft_minus_ft0
                        * (a[10] as f64
                            + ft_minus_ft0 * (a[11] as f64 + ft_minus_ft0 * a[15] as f64));

                let p = k0 + presf_minus_fp0 * (k1 + presf_minus_fp0 * (k2 + presf_minus_fp0 * k3));

                p
            }
            _ => f64::NAN,
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct T5Coeffs {
    pub f0: f32,
    pub t0: f32,
    pub c: [f32; crate::protobuf::T_COEFFS_COUNT],
}

impl T5Coeffs {
    pub fn calc(&self, f: Option<f64>) -> f64 {
        if let Some(f) = f {
            let temp_f_minus_fp0 = f - self.f0 as f64;
            let mut result = self.t0 as f64;
            let mut mu = temp_f_minus_fp0;

            for i in 0..crate::protobuf::T_COEFFS_COUNT {
                result += mu * self.c[i] as f64;
                mu *= temp_f_minus_fp0;
            }

            result
        } else {
            f64::NAN
        }
    }
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
#[derive(Debug, Copy, Clone, Serialize, Default, PartialEq, defmt::Format)]
#[serde(rename_all = "PascalCase")]
pub struct Monitoring {
    pub overpress: bool,
    pub overheat: bool,
    pub cpu_overheat: bool,
    pub over_power: bool,
}

impl Monitoring {
    pub fn is_set(&self) -> bool {
        self.overpress | self.overheat | self.cpu_overheat | self.over_power
    }

    pub fn has_new_flags(&self, other: &Monitoring) -> bool {
        if !self.overpress && other.overpress {
            return true;
        }
        if !self.overheat && other.overheat {
            return true;
        }
        if !self.cpu_overheat && other.cpu_overheat {
            return true;
        }
        if !self.over_power && other.over_power {
            return true;
        }

        false
    }
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

impl PressureMeassureUnits {
    pub fn wrap(&self, value: f64) -> f64 {
        let multiplier = match self {
            PressureMeassureUnits::InvalidZero => 0.0,
            PressureMeassureUnits::Pa => 100000.0,
            PressureMeassureUnits::Bar => 1.0,
            PressureMeassureUnits::At => 1.0197162,
            PressureMeassureUnits::MmH20 => 10197.162,
            PressureMeassureUnits::MHg => 750.06158 / 1000.0,
            PressureMeassureUnits::Atm => 0.98692327,
            PressureMeassureUnits::PSI => 14.5,
        };

        value * multiplier
    }
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
    pub password: [u8; crate::protobuf::PASSWORD_SIZE],

    pub monitoring: Monitoring,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct NonStoreSettings {
    pub current_password: [u8; crate::protobuf::PASSWORD_SIZE],
}
