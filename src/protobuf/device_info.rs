use my_proc_macro::git_version;

use crate::{
    settings,
    workmodes::{FChannel, output_storage::OutputStorage},
};

pub fn fill_info(
    info: &mut super::messages::InfoResponse,
    output_data: &OutputStorage,
    with_settings: &mut impl FnMut(
        &mut dyn FnMut(&mut (settings::AppSettings, settings::NonStoreSettings)) -> (bool, bool),
    ) -> bool,
) {
    info.hw_version = crate::config::HW_VERSION;
    info.sw_version = git_version!();

    info.pressure_channel_failed = output_data.frequencys[FChannel::Pressure as usize].is_none();
    info.temperature_channel_failed =
        output_data.frequencys[FChannel::Temperature as usize].is_none();

    with_settings(&mut |(app_settings, _)| {
        info.overpress_detected = app_settings.monitoring.overpress;
        info.overheat_detected = app_settings.monitoring.overheat;
        info.overheat_cpu_detected = app_settings.monitoring.cpu_overheat;
        info.over_vbat_detected = app_settings.monitoring.over_power;

        (false, false)
    });
}
