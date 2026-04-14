include!(concat!(
    env!("OUT_DIR"),
    "/ru.sktbelpa.pressure_self_writer.rs"
));

pub const P_COEFFS_COUNT: usize = 16;
pub const T_COEFFS_COUNT: usize = 5;
pub const PASSWORD_SIZE: usize = 10;

impl From<&crate::settings::P16Coeffs> for PCoefficients {
    fn from(p_coeffs: &crate::settings::P16Coeffs) -> Self {
        Self {
            ft0: Some(p_coeffs.fp0),
            fp0: Some(p_coeffs.ft0),

            a0: Some(p_coeffs.a[0]),
            a1: Some(p_coeffs.a[1]),
            a2: Some(p_coeffs.a[2]),
            a3: Some(p_coeffs.a[3]),
            a4: Some(p_coeffs.a[4]),
            a5: Some(p_coeffs.a[5]),
            a6: Some(p_coeffs.a[6]),
            a7: Some(p_coeffs.a[7]),
            a8: Some(p_coeffs.a[8]),
            a9: Some(p_coeffs.a[9]),
            a10: Some(p_coeffs.a[10]),
            a11: Some(p_coeffs.a[11]),
            a12: Some(p_coeffs.a[12]),
            a13: Some(p_coeffs.a[13]),
            a14: Some(p_coeffs.a[14]),
            a15: Some(p_coeffs.a[15]),
        }
    }
}

impl From<&crate::settings::T5Coeffs> for T5Coefficients {
    fn from(t_coeffs: &crate::settings::T5Coeffs) -> Self {
        Self {
            t0: Some(t_coeffs.t0),
            f0: Some(t_coeffs.f0),

            c1: Some(t_coeffs.c[0]),
            c2: Some(t_coeffs.c[1]),
            c3: Some(t_coeffs.c[2]),
            c4: Some(t_coeffs.c[3]),
            c5: Some(t_coeffs.c[4]),
        }
    }
}

impl From<&crate::settings::WorkRange> for WorkRange {
    fn from(wr: &crate::settings::WorkRange) -> Self {
        Self {
            minimum: Some(wr.minimum),
            maximum: Some(wr.maximum),
            absolute_maximum: Some(wr.absolute_maximum),
        }
    }
}

impl WorkRange {
    pub(crate) fn validate(&self) -> Result<(), WorkRangeError> {
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

impl From<&crate::settings::CalibrationDate> for CalibrationDate {
    fn from(cd: &crate::settings::CalibrationDate) -> Self {
        Self {
            day: Some(cd.day),
            month: Some(cd.month),
            year: Some(cd.year),
        }
    }
}

impl CalibrationDate {
    pub fn validate(&self) -> Result<(), DateField> {
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

impl From<&crate::settings::WriteConfig> for WriteConfig {
    fn from(wc: &crate::settings::WriteConfig) -> Self {
        Self {
            base_interval_ms: Some(wc.base_interval_ms),
            p_write_devider: Some(wc.p_write_devider),
            t_write_devider: Some(wc.t_write_devider),
        }
    }
}
