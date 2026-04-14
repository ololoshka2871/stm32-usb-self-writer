use alloc::string::{String, ToString};

use crate::settings::{self, SettingActionError};

pub fn reset_monitoring_flags(
    with_settings: &mut impl FnMut(
        &mut dyn FnMut(&mut (settings::AppSettings, settings::NonStoreSettings)) -> (bool, bool),
    ) -> bool,
) -> Result<bool, SettingActionError<String>> {
    let mut err = None;

    let res = with_settings(&mut |(ws, ts)| {
        if ws.password != ts.current_password {
            let need_store = ws.monitoring.is_set();
            ws.monitoring = settings::Monitoring::default();

            (need_store, need_store)
        } else {
            err = Some("Invalid password".to_string());
            (false, false)
        }
    });

    if let Some(e) = err {
        Err(SettingActionError::new(e))
    } else {
        Ok(res)
    }
}
