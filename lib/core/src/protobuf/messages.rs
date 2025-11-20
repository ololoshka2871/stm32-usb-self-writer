include!(concat!(
    env!("OUT_DIR"),
    "/ru.sktbelpa.pressure_self_writer_p2t.rs"
));

use crate::app_settings;

pub const P_COEFFS_COUNT: usize = 16;
pub const T_COEFFS_COUNT: usize = 5;
pub const PASSWORD_SIZE: usize = 10;

pub trait Validator<T> {
    fn validate(&self) -> Result<(), T>;
}

impl From<&app_settings::P16Coeffs> for PCoefficients {
    fn from(p_coeffs: &app_settings::P16Coeffs) -> Self {
        Self {
            ft0: Some(p_coeffs.Fp0),
            fp0: Some(p_coeffs.Ft0),

            a0: Some(p_coeffs.A[0]),
            a1: Some(p_coeffs.A[1]),
            a2: Some(p_coeffs.A[2]),
            a3: Some(p_coeffs.A[3]),
            a4: Some(p_coeffs.A[4]),
            a5: Some(p_coeffs.A[5]),
            a6: Some(p_coeffs.A[6]),
            a7: Some(p_coeffs.A[7]),
            a8: Some(p_coeffs.A[8]),
            a9: Some(p_coeffs.A[9]),
            a10: Some(p_coeffs.A[10]),
            a11: Some(p_coeffs.A[11]),
            a12: Some(p_coeffs.A[12]),
            a13: Some(p_coeffs.A[13]),
            a14: Some(p_coeffs.A[14]),
            a15: Some(p_coeffs.A[15]),
        }
    }
}

impl From<&app_settings::T5Coeffs> for T5Coefficients {
    fn from(t_coeffs: &app_settings::T5Coeffs) -> Self {
        Self {
            t0: Some(t_coeffs.T0),
            f0: Some(t_coeffs.F0),

            c1: Some(t_coeffs.C[0]),
            c2: Some(t_coeffs.C[1]),
            c3: Some(t_coeffs.C[2]),
            c4: Some(t_coeffs.C[3]),
            c5: Some(t_coeffs.C[4]),
        }
    }
}

impl From<&app_settings::WorkRange> for WorkRange {
    fn from(wr: &app_settings::WorkRange) -> Self {
        Self {
            minimum: Some(wr.minimum),
            maximum: Some(wr.maximum),
            absolute_maximum: Some(wr.absolute_maximum),
        }
    }
}

impl Validator<WorkRangeError> for WorkRange {
    fn validate(&self) -> Result<(), WorkRangeError> {
        if let Some(absolute_maximum) = self.absolute_maximum {
            if self.maximum.is_some() && absolute_maximum < self.maximum.unwrap_or_default() {
                return Err(WorkRangeError::MaximumAboveAbasoluteMaximum);
            }
            if self.minimum.is_some() && absolute_maximum < self.minimum.unwrap_or_default() {
                return Err(WorkRangeError::MinimumAboveAbsoluteMaximum);
            }
        }

        if self.maximum.is_some()
            && self.minimum.is_some()
            && (self.maximum.unwrap_or_default() < self.minimum.unwrap_or_default())
        {
            return Err(WorkRangeError::MinimumAboveMaximum);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum DateField {
    Day,
    Month,
    Past,
}

impl From<&app_settings::CalibrationDate> for CalibrationDate {
    fn from(cd: &app_settings::CalibrationDate) -> Self {
        Self {
            day: Some(cd.Day),
            month: Some(cd.Month),
            year: Some(cd.Year),
        }
    }
}

impl Validator<DateField> for CalibrationDate {
    fn validate(&self) -> Result<(), DateField> {
        use my_proc_macro::{build_day, build_month, build_year};

        if let Some(day) = self.day {
            if day > 31 {
                return Err(DateField::Day);
            }
        }
        if let Some(month) = self.month {
            if month > 12 || month < 1 {
                return Err(DateField::Month);
            }
        }
        if let Some(year) = self.year {
            if year < build_year!() {
                return Err(DateField::Past);
            }
            if self.day.is_some() && self.month.is_some() {
                if self.month.unwrap_or_default() < build_month!() {
                    return Err(DateField::Past);
                } else if self.month.unwrap_or_default() == build_month!()
                    && self.day.unwrap_or_default() < build_day!()
                {
                    return Err(DateField::Past);
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum WorkRangeError {
    MinimumAboveMaximum,
    MinimumAboveAbsoluteMaximum,
    MaximumAboveAbasoluteMaximum,
}

impl From<&app_settings::WriteConfig> for WriteConfig {
    fn from(wc: &app_settings::WriteConfig) -> Self {
        Self {
            base_interval_ms: Some(wc.BaseInterval_ms),
            p_write_devider: Some(wc.PWriteDevider),
            t_write_devider: Some(wc.TWriteDevider),
        }
    }
}
