use crate::{
    main_data_storage::StorageMetaHandle,
    settings,
    workmodes::output_storage::OutputStorage,
};

const PROTOCOL_VERSION: u32 = super::messages::Info::ProtocolVersion as u32;

pub fn process_request(
    req: &super::messages::Request,
    resp: &mut super::messages::Response,
    output_getter: &mut impl FnMut() -> OutputStorage,
    with_settings: &mut impl FnMut(
        &mut dyn FnMut(&mut (settings::AppSettings, settings::NonStoreSettings)) -> (bool, bool),
    ) -> bool,
    storage_meta: StorageMetaHandle,
) -> bool {
    if !(req.device_id == super::messages::Info::PressureSelfWriterId as u32
        || req.device_id == super::messages::Info::IdDiscover as u32)
    {
        defmt::error!("Protobuf: unknown target device id: 0x{:X}", req.device_id);

        resp.global_status = super::messages::Status::ProtocolError as i32;
        return false;
    }

    match req.protocol_version {
        PROTOCOL_VERSION | 0 => (),
        v => {
            defmt::warn!("Protobuf: unsupported protocol version {}", v);
            resp.global_status = super::messages::Status::ProtocolError as i32;
            return false;
        }
    }

    let mut need_to_write_settings = false;

    if let Some(write_settings) = &req.write_settings {
        match super::process_settings::update_settings(&write_settings, with_settings) {
            Ok(need_to_write) => need_to_write_settings = need_to_write,
            Err(e) => {
                defmt::error!("Set settings error: {}", e);
                resp.global_status = super::messages::Status::ErrorsInSubcommands as i32;
            }
        }
        let mut get_settings = super::messages::SettingsResponse::default();
        super::process_settings::fill_settings(&mut get_settings, with_settings);
        resp.get_settings = Some(get_settings);
    }

    if req.get_info.is_some() {
        let mut info = super::messages::InfoResponse::default();
        let output_data = output_getter();
        super::device_info::fill_info(&mut info, &output_data, with_settings);
        resp.info = Some(info);
    }

    if let Some(change_password) = &req.change_password {
        let password_changed =
            match super::change_password::change_password(change_password, with_settings) {
                Err(e) => {
                    defmt::error!("Failed to change password: {}", e);
                    resp.global_status = super::messages::Status::ErrorsInSubcommands as i32;
                    false
                }
                Ok(need_to_write) => {
                    need_to_write_settings = need_to_write;
                    true
                }
            };

        resp.change_password_status =
            Some(super::messages::ChangePasswordStatus { password_changed });
    }

    if let Some(flash_command) = req.flash_command {
        let mut flash_status = super::messages::FlashStatus::default();

        let mut reset_monitoring_failed = None;
        if flash_command.reset_monitoring.is_some() {
            defmt::warn!("Reseting monitoring flags!");
            reset_monitoring_failed = if let Err(e) =
                crate::protobuf::monitoring_over_conditions::reset_monitoring_flags(with_settings)
            {
                defmt::error!("Failed to reset monitoring: {}", e);
                resp.global_status = super::messages::Status::ErrorsInSubcommands as i32;
                Some(true)
            } else {
                need_to_write_settings = true;
                Some(false)
            }
        }

        let mut clear_memory_requested = false;
        if let Some(true) = flash_command.clear_memory {
            defmt::warn!("Start clearing memory!");
            match storage_meta.request_erase() {
                Ok(()) => clear_memory_requested = true,
                Err(e) => {
                    defmt::error!(
                        "Failed to start clear memory: {}",
                        defmt::Debug2Format(&e)
                    );
                    resp.global_status = super::messages::Status::ErrorsInSubcommands as i32;
                }
            }
        }

        fill_flash_state(
            &mut flash_status,
            storage_meta,
            reset_monitoring_failed,
            clear_memory_requested,
        );

        resp.flash_status = Some(flash_status);
    }

    if let Some(req) = req.get_output_values {
        let mut out = super::messages::OutputResponse::default();
        let output_data = output_getter();
        super::output::fill_output(&mut out, &req, &output_data);
        resp.output = Some(out);
    }

    need_to_write_settings
}

fn fill_flash_state(
    flash_status: &mut super::messages::FlashStatus,
    storage_meta: StorageMetaHandle,
    reset_monitoring_failed: Option<bool>,
    clear_memory_requested: bool,
) {
    flash_status.flash_page_size = storage_meta.block_size_bytes();
    flash_status.flash_pages = storage_meta.total_blocks();
    flash_status.flash_used_pages = storage_meta.used_blocks();

    flash_status.status = if let Some(true) = reset_monitoring_failed {
        super::messages::flash_status::Status::ResetMonitoringFailed
    } else if clear_memory_requested || storage_meta.erase_in_progress() {
        super::messages::flash_status::Status::Ereasing
    } else {
        super::messages::flash_status::Status::Ok
    } as i32;
}
