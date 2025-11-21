use my_proc_macro::git_version;

use crate::threads::sensor_processor::FChannel;
use corelogic::output_provider::OutputProviderError;

pub fn fill_info<E>(
    info: &mut super::messages::InfoResponse,
    output_provider: &impl corelogic::output_provider::OutputProvider<
        Output = crate::workmodes::output_storage::OutputStorage,
        Error = E,
    >,
) -> Result<(), OutputProviderError<E>> {
    info.hw_version = crate::config::HW_VERSION;
    info.sw_version = git_version!();

    let mut err: Option<OutputProviderError<E>> = None;

    match output_provider.with_output(|guard| {
        info.pressure_channel_failed = guard.frequencys[FChannel::Pressure as usize].is_none();
        info.temperature_channel_failed =
            guard.frequencys[FChannel::Temperature1 as usize].is_none();
    }) {
        Ok(()) => {}
        Err(e) => {
            info.pressure_channel_failed = false;
            info.temperature_channel_failed = false;
            err = Some(OutputProviderError::Other(e));
        }
    }

    let r: Result<(), crate::settings::SettingActionError<()>> = crate::settings::settings_action(
        crate::protobuf::OUT_STORAGE_LOCK_WAIT.clone(),
        |(app_settings, _)| {
            info.overpress_detected = app_settings.monitoring.Ovarpress;
            info.overheat_detected = app_settings.monitoring.Ovarheat;
            info.overheat_cpu_detected = app_settings.monitoring.CPUOvarheat;
            info.over_vbat_detected = app_settings.monitoring.OverPower;
            Ok(())
        },
    );

    if r.is_err() {
        err = Some(OutputProviderError::LockFailed);
    }

    if let Some(e) = err {
        Err(e)
    } else {
        Ok(())
    }
}
