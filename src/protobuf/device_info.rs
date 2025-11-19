use alloc::sync::Arc;
use freertos_rust::{Duration, FreeRtosError, Mutex};
use my_proc_macro::git_version;

use crate::{threads::sensor_processor::FChannel, workmodes::output_storage::OutputStorage};

pub fn fill_info(
    info: &mut super::messages::InfoResponse,
    output_storage: &Arc<Mutex<OutputStorage>>,
) -> Result<(), FreeRtosError> {
    let mut err = None;

    info.hw_version = crate::config::HW_VERSION;
    info.sw_version = git_version!();

    match output_storage.lock(*super::OUT_STORAGE_LOCK_WAIT) {
        Ok(guard) => {
            info.pressure_channel_failed = guard.frequencys[FChannel::Pressure as usize].is_none();
            info.temperature_channel_failed =
                guard.frequencys[FChannel::Temperature1 as usize].is_none();
        }
        Err(e) => {
            info.pressure_channel_failed = false;
            info.temperature_channel_failed = false;
            err = Some(e);
        }
    }

    let r: Result<(), crate::settings::SettingActionError<()>> =
        crate::settings::settings_action(Duration::ms(1), |(app_settings, _)| {
            info.overpress_detected = app_settings.monitoring.Ovarpress;
            info.overheat_detected = app_settings.monitoring.Ovarheat;
            info.overheat_cpu_detected = app_settings.monitoring.CPUOvarheat;
            info.over_vbat_detected = app_settings.monitoring.OverPower;
            Ok(())
        });

    if r.is_err() {
        err = Some(FreeRtosError::Timeout);
    }

    if let Some(e) = err {
        Err(e)
    } else {
        Ok(())
    }
}
