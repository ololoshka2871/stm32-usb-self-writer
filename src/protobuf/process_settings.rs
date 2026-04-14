use core::usize;

use alloc::{
    format,
    string::{String, ToString},
};

use my_proc_macro::store_coeff;

use crate::{
    config,
    protobuf::PASSWORD_SIZE,
    settings::{self, SettingActionError},
};

use super::messages;

const F_REF_DELTA: u32 = 500;

fn strlenn(str: &[u8], max: usize) -> usize {
    let max_scan = core::cmp::min(str.len(), max);
    for i in 0..max_scan {
        if str[i] == b'\0' {
            return i;
        }
    }
    max
}

pub fn fill_settings(
    settings_resp: &mut messages::SettingsResponse,
    with_settings: &mut impl FnMut(
        &mut dyn FnMut(&mut (settings::AppSettings, settings::NonStoreSettings)) -> (bool, bool),
    ) -> bool,
) {
    with_settings(&mut |(ws, ts)| {
        settings_resp.serial = ws.serial;

        settings_resp.fref = ws.fref;

        settings_resp.p_coefficients = (&ws.p_coefficients).into();
        settings_resp.t_coefficients = (&ws.t_coefficients).into();

        settings_resp.p_work_range = (&ws.p_work_range).into();
        settings_resp.t_work_range = (&ws.t_work_range).into();
        settings_resp.tcpu_work_range = (&ws.t_cpu_work_range).into();
        settings_resp.bat_work_range = (&ws.vbat_work_range).into();

        settings_resp.calibration_date = (&ws.calibration_date).into();

        settings_resp.p_zero_correction = ws.p_zero_correction;
        settings_resp.t_zero_correction = ws.t_zero_correction;

        settings_resp.write_config = (&ws.write_config).into();

        settings_resp.start_delay = ws.start_delay;

        settings_resp.pressure_meassure_units = ws.pressure_meassure_units as i32;

        settings_resp.password = String::from_utf8_lossy(
            &ts.current_password[..strlenn(&ts.current_password, PASSWORD_SIZE)],
        )
        .to_string();

        (false, false)
    });
}

fn verify_parameters(
    ws: &super::messages::WriteSettingsReq,
    with_settings: &mut impl FnMut(
        &mut dyn FnMut(&mut (settings::AppSettings, settings::NonStoreSettings)) -> (bool, bool),
    ) -> bool,
) -> Result<(), SettingActionError<String>> {
    let mut password_invalid = false;
    with_settings(&mut |(ws, ts)| {
        password_invalid = ws.password != ts.current_password;
        (false, false)
    });

    let deny_if_password_invalid = move |parameter: &str| {
        if password_invalid {
            Err(SettingActionError::new(format!(
                "Change {}, invalid password",
                parameter
            )))
        } else {
            Ok(())
        }
    };

    if ws.set_serial.is_some() {
        deny_if_password_invalid("Serial")?;
    }

    if let Some(set_fref) = ws.set_fref {
        deny_if_password_invalid("Fref")?;
        if set_fref > config::XTAL_FREQ + F_REF_DELTA || set_fref < config::XTAL_FREQ - F_REF_DELTA
        {
            return Err(SettingActionError::new(format!(
                "Reference frequency {} is too different from base {} +/- {}",
                set_fref,
                config::XTAL_FREQ,
                F_REF_DELTA
            )));
        }
    }

    if let Some(set_p_coefficients) = &ws.set_p_coefficients {
        if set_p_coefficients.a0.is_some()
            || set_p_coefficients.a1.is_some()
            || set_p_coefficients.a2.is_some()
            || set_p_coefficients.a3.is_some()
            || set_p_coefficients.a4.is_some()
            || set_p_coefficients.a5.is_some()
            || set_p_coefficients.a6.is_some()
            || set_p_coefficients.a7.is_some()
            || set_p_coefficients.a8.is_some()
            || set_p_coefficients.a9.is_some()
            || set_p_coefficients.a10.is_some()
            || set_p_coefficients.a11.is_some()
            || set_p_coefficients.a12.is_some()
            || set_p_coefficients.a13.is_some()
            || set_p_coefficients.a14.is_some()
            || set_p_coefficients.a15.is_some()
            || set_p_coefficients.ft0.is_some()
            || set_p_coefficients.fp0.is_some()
        {
            deny_if_password_invalid("PCoefficients")?;
        }
    }

    if let Some(set_t_coefficients) = &ws.set_t_coefficients {
        if set_t_coefficients.t0.is_some()
            || set_t_coefficients.c1.is_some()
            || set_t_coefficients.c2.is_some()
            || set_t_coefficients.c3.is_some()
            || set_t_coefficients.c4.is_some()
            || set_t_coefficients.c5.is_some()
            || set_t_coefficients.f0.is_some()
        {
            deny_if_password_invalid("TCoefficients")?;
        }
    }

    if let Some(set_p_work_range) = &ws.set_p_work_range {
        if set_p_work_range.minimum.is_some()
            || set_p_work_range.maximum.is_some()
            || set_p_work_range.absolute_maximum.is_some()
        {
            deny_if_password_invalid("PWorkRange")?;

            set_p_work_range
                .validate()
                .map_err(|e| SettingActionError::new(format!("PWorkRange invalid: {:?}", e)))?;
        }
    }

    if let Some(set_t_work_range) = &ws.set_t_work_range {
        if set_t_work_range.minimum.is_some()
            || set_t_work_range.maximum.is_some()
            || set_t_work_range.absolute_maximum.is_some()
        {
            deny_if_password_invalid("TWorkRange")?;

            set_t_work_range
                .validate()
                .map_err(|e| SettingActionError::new(format!("TWorkRange invalid: {:?}", e)))?;
        }
    }

    if let Some(set_tcpu_work_range) = &ws.set_tcpu_work_range {
        if set_tcpu_work_range.minimum.is_some()
            || set_tcpu_work_range.maximum.is_some()
            || set_tcpu_work_range.absolute_maximum.is_some()
        {
            deny_if_password_invalid("TWorkRange")?;

            set_tcpu_work_range
                .validate()
                .map_err(|e| SettingActionError::new(format!("TCPUWorkRange invalid: {:?}", e)))?;
        }
    }

    if let Some(set_bat_work_range) = &ws.set_bat_work_range {
        if set_bat_work_range.minimum.is_some()
            || set_bat_work_range.maximum.is_some()
            || set_bat_work_range.absolute_maximum.is_some()
        {
            deny_if_password_invalid("TWorkRange")?;

            set_bat_work_range
                .validate()
                .map_err(|e| SettingActionError::new(format!("BatWorkRange invalid: {:?}", e)))?;
        }
    }

    if let Some(set_calibration_date) = &ws.set_calibration_date {
        set_calibration_date.validate().map_err(|e| {
            SettingActionError::new(format!("Calibration date field {:?} invalid", e))
        })?;
    }

    if let Some(set_write_config) = &ws.set_write_config {
        if let Some(base_interval_ms) = set_write_config.base_interval_ms {
            if base_interval_ms < config::BASE_INTERVAL_MIN_MS {
                return Err(SettingActionError::new(format!(
                    "Write base period {} too small, min={}",
                    base_interval_ms,
                    config::BASE_INTERVAL_MIN_MS
                )));
            }
        }
        if let Some(p_devider) = set_write_config.p_write_devider {
            if p_devider == 0 {
                return Err(SettingActionError::new("P write devider == 0".to_string()));
            }
        }
        if let Some(t_devider) = set_write_config.t_write_devider {
            if t_devider == 0 {
                return Err(SettingActionError::new("T write devider == 0".to_string()));
            }
        }
    }

    if let Some(set_pressure_meassure_units) = ws.set_pressure_meassure_units {
        if let Some(settings::PressureMeassureUnits::InvalidZero) | None =
            num::FromPrimitive::from_i32(set_pressure_meassure_units)
        {
            return Err(SettingActionError::new(format!(
                "Value {} is not a valid pressure measure unit code.",
                set_pressure_meassure_units
            )));
        }
    }

    Ok(())
}

pub fn update_settings(
    w: &super::messages::WriteSettingsReq,
    with_settings: &mut impl FnMut(
        &mut dyn FnMut(&mut (settings::AppSettings, settings::NonStoreSettings)) -> (bool, bool),
    ) -> bool,
) -> Result<bool, SettingActionError<String>> {
    //use crate::threads::sensor_processor::{AChannel, FChannel};

    verify_parameters(w, with_settings)?;

    let mut err = None;
    let res = with_settings(&mut |(ws, ts)| {
        let mut need_write = false;

        // store_coeff!() раскладывается в ->
        /*
        w.set_serial.map(|v| {
            ws.Serial = v;
            need_write = true;
        });
        */

        store_coeff!(ws.serial <= w; set_serial; need_write);

        store_coeff!(ws.fref <= w; set_fref; need_write);

        if let Some(set_p_coefficients) = &w.set_p_coefficients {
            store_coeff!(ws.p_coefficients.fp0 <= set_p_coefficients; fp0; need_write);
            store_coeff!(ws.p_coefficients.ft0 <= set_p_coefficients; ft0; need_write);
            store_coeff!(ws.p_coefficients.a[0] <= set_p_coefficients; a0; need_write);
            store_coeff!(ws.p_coefficients.a[1] <= set_p_coefficients; a1; need_write);
            store_coeff!(ws.p_coefficients.a[2] <= set_p_coefficients; a2; need_write);
            store_coeff!(ws.p_coefficients.a[3] <= set_p_coefficients; a3; need_write);
            store_coeff!(ws.p_coefficients.a[4] <= set_p_coefficients; a4; need_write);
            store_coeff!(ws.p_coefficients.a[5] <= set_p_coefficients; a5; need_write);
            store_coeff!(ws.p_coefficients.a[6] <= set_p_coefficients; a6; need_write);
            store_coeff!(ws.p_coefficients.a[7] <= set_p_coefficients; a7; need_write);
            store_coeff!(ws.p_coefficients.a[8] <= set_p_coefficients; a8; need_write);
            store_coeff!(ws.p_coefficients.a[9] <= set_p_coefficients; a9; need_write);
            store_coeff!(ws.p_coefficients.a[10] <= set_p_coefficients; a10; need_write);
            store_coeff!(ws.p_coefficients.a[11] <= set_p_coefficients; a11; need_write);
            store_coeff!(ws.p_coefficients.a[12] <= set_p_coefficients; a12; need_write);
            store_coeff!(ws.p_coefficients.a[13] <= set_p_coefficients; a13; need_write);
            store_coeff!(ws.p_coefficients.a[14] <= set_p_coefficients; a14; need_write);
            store_coeff!(ws.p_coefficients.a[15] <= set_p_coefficients; a15; need_write);
        }

        if let Some(set_t_coefficients) = &w.set_t_coefficients {
            store_coeff!(ws.t_coefficients.f0 <= set_t_coefficients; f0; need_write);
            store_coeff!(ws.t_coefficients.c[0] <= set_t_coefficients; c1; need_write);
            store_coeff!(ws.t_coefficients.c[1] <= set_t_coefficients; c2; need_write);
            store_coeff!(ws.t_coefficients.c[2] <= set_t_coefficients; c3; need_write);
            store_coeff!(ws.t_coefficients.c[3] <= set_t_coefficients; c4; need_write);
            store_coeff!(ws.t_coefficients.c[4] <= set_t_coefficients; c5; need_write);
            store_coeff!(ws.t_coefficients.t0 <= set_t_coefficients; t0; need_write);
        }

        if let Some(set_p_work_range) = &w.set_p_work_range {
            store_coeff!(ws.p_work_range.minimum <= set_p_work_range; minimum; need_write);
            store_coeff!(ws.p_work_range.maximum <= set_p_work_range; maximum; need_write);
            store_coeff!(ws.p_work_range.absolute_maximum <= set_p_work_range; absolute_maximum; need_write);
        }
        if let Some(set_t_work_range) = &w.set_t_work_range {
            store_coeff!(ws.t_work_range.minimum <= set_t_work_range; minimum; need_write);
            store_coeff!(ws.t_work_range.maximum <= set_t_work_range; maximum; need_write);
            store_coeff!(ws.t_work_range.absolute_maximum <= set_t_work_range; absolute_maximum; need_write);
        }
        if let Some(set_tcpu_work_range) = &w.set_tcpu_work_range {
            store_coeff!(ws.t_cpu_work_range.minimum <= set_tcpu_work_range; minimum; need_write);
            store_coeff!(ws.t_cpu_work_range.maximum <= set_tcpu_work_range; maximum; need_write);
            store_coeff!(ws.t_cpu_work_range.absolute_maximum <= set_tcpu_work_range; absolute_maximum; need_write);
        }
        if let Some(set_bat_work_range) = &w.set_bat_work_range {
            store_coeff!(ws.vbat_work_range.minimum <= set_bat_work_range; minimum; need_write);
            store_coeff!(ws.vbat_work_range.maximum <= set_bat_work_range; maximum; need_write);
            store_coeff!(ws.vbat_work_range.absolute_maximum <= set_bat_work_range; absolute_maximum; need_write);
        }

        if let Some(set_calibration_date) = &w.set_calibration_date {
            store_coeff!(ws.calibration_date.day <= set_calibration_date; day; need_write);
            store_coeff!(ws.calibration_date.month <= set_calibration_date; month; need_write);
            store_coeff!(ws.calibration_date.year <= set_calibration_date; year; need_write);
        }

        store_coeff!(ws.p_zero_correction <= w; set_p_zero_correction; need_write);
        store_coeff!(ws.t_zero_correction <= w; set_t_zero_correction; need_write);

        if let Some(set_write_config) = &w.set_write_config {
            store_coeff!(ws.write_config.base_interval_ms <= set_write_config; base_interval_ms; need_write);
            store_coeff!(ws.write_config.p_write_devider <= set_write_config; p_write_devider; need_write);
            store_coeff!(ws.write_config.t_write_devider <= set_write_config; t_write_devider; need_write);
        }

        store_coeff!(ws.start_delay <= w; set_start_delay; need_write);

        if let Some(set_pressure_meassure_units) = w.set_pressure_meassure_units {
            if let Some(mu) = num::FromPrimitive::from_i32(set_pressure_meassure_units) {
                ws.pressure_meassure_units = mu;
                need_write = true;
            } else {
                err = Some("Invalid measure unit".to_string());
            }
        }

        let mut password_set = false;
        if let Some(set_password) = &w.set_password {
            let newlen = core::cmp::min(set_password.len(), PASSWORD_SIZE);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    set_password.as_ptr(),
                    ts.current_password.as_mut_ptr(),
                    newlen,
                );
            }
            ts.current_password[newlen..].fill(b'\0');
            password_set = true;
        }

        (need_write | password_set, need_write)
    });

    if let Some(e) = err {
        Err(SettingActionError::new(e))
    } else {
        Ok(res)
    }
}
