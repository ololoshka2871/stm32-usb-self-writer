use alloc::string::{String, ToString};

use crate::settings::{self, SettingActionError};

use super::PASSWORD_SIZE;

pub fn change_password(
    cmd: &super::messages::ChangePassword,
    with_settings: &mut impl FnMut(
        &mut dyn FnMut(&mut (settings::AppSettings, settings::NonStoreSettings)) -> (bool, bool),
    ) -> bool,
) -> Result<bool, SettingActionError<String>> {
    let mut pass = [0u8; PASSWORD_SIZE];

    pass[..cmd.new_password.len()].copy_from_slice(&cmd.new_password.as_bytes());

    let mut error = None;
    let need_to_write = with_settings(&mut |(app_settings, ts)| {
        if app_settings.password != ts.current_password {
            error = Some("Invalid password".to_string());
        } else if ts.current_password == pass {
            /* new password is the same as the current one, skip */
        } else {
            app_settings.password.copy_from_slice(&pass);
            ts.current_password.copy_from_slice(&pass);
            return (true, true);
        }

        (false, false)
    });

    if let Some(e) = error {
        Err(SettingActionError::new(e))
    } else {
        Ok(need_to_write)
    }
}
