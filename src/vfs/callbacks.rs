#![allow(dead_code, unsafe_op_in_unsafe_fn)]

use core::cell::RefCell;

use alloc::{boxed::Box, string::String, sync::Arc};

use heatshrink_rust::{CompressedData, decoder::HeatshrinkDecoder};

use super::StaticBinData;

#[repr(C)]
pub(crate) struct FlashReadUserData {
    pub storage_context: *const crate::main_data_storage::StorageContext,
    pub used_view: bool,
}

pub(crate) unsafe extern "C" fn const_binary_reader(
    dest: *mut u8,
    size: i32,
    offset: u32,
    userdata: usize,
) {
    let dptr = &*(userdata as *const StaticBinData);
    if offset as usize > dptr.data.len() {
        return;
    }
    let to_read = if offset as usize + size as usize > dptr.data.len() {
        dptr.data.len() - offset as usize
    } else {
        size as usize
    };

    core::ptr::copy_nonoverlapping(dptr.data.as_ptr().add(offset as usize), dest, to_read);
}

pub(crate) unsafe extern "C" fn unpack_reader(
    dest: *mut u8,
    size: i32,
    offset: u32,
    userdata: usize,
) {
    let dptr = &*(userdata as *const CompressedData);
    if offset as usize > dptr.original_size {
        return;
    }
    let to_read = if (offset as usize + size as usize) > dptr.original_size {
        dptr.original_size - offset as usize
    } else {
        size as usize
    };

    HeatshrinkDecoder::source(dptr.data.iter().cloned())
        .skip(offset as usize)
        .take(to_read)
        .enumerate()
        .for_each(|(n, d)| *dest.add(n) = d);
}

pub(crate) unsafe extern "C" fn null_read(
    _dest: *mut u8,
    _size: i32,
    _offset: u32,
    _userdata: usize,
) {
}

pub(crate) unsafe fn store_block_data(s: String, dest: *mut u8, size: i32, offset: u32) {
    let src = s.as_bytes();
    let offset = offset as usize;
    if src.len() > offset {
        let src = &src[offset..];
        let to_write = core::cmp::min(size as usize, src.len());
        core::ptr::copy_nonoverlapping(src.as_ptr(), dest, to_write);

        // забиваем буфер пробелами до конца, чтобы в блокноте он нормально выглядел
        core::ptr::write_bytes(dest.add(src.len()), b' ', size as usize - to_write);
    } else {
        // все пробелами забить
        core::ptr::write_bytes(dest, b' ', size as usize);
    }
}

pub(crate) unsafe extern "C" fn settings_read(
    dest: *mut u8,
    size: i32,
    offset: u32,
    userdata: usize,
) {
    let settings_accessor =
        &*(userdata as *const Arc<RefCell<Option<Box<super::SettingsAccessor>>>>);

    if let Some(accessor) = settings_accessor.borrow().as_ref() {
        let s = accessor();
        match serde_json::to_string_pretty(&s) {
            Ok(s) => {
                store_block_data(s, dest, size, offset);
                return;
            }
            Err(e) => {
                defmt::error!(
                    "Failed to serialise settings: {}",
                    defmt::Display2Format(&e)
                );
            }
        }
    } else {
        defmt::error!("Settings are not available");
    }
    // забиваем буфер пробелами, чтобы в блокноте он нормально выглядел
    core::ptr::write_bytes(dest, b' ', size as usize);
}

pub(crate) unsafe extern "C" fn meminfo_read(
    dest: *mut u8,
    size: i32,
    offset: u32,
    userdata: usize,
) {
    use serde::Serialize;

    #[derive(Serialize)]
    #[serde(rename_all = "PascalCase")]
    struct MemInfo {
        pub block_size_bytes: u32,
        pub total_blocks: u32,
        pub used_blocks: u32,
        pub freq_multiplier: u32,
    }

    if userdata == 0 {
        core::ptr::write_bytes(dest, b' ', size as usize);
        return;
    }

    let storage_context = &*(userdata as *const crate::main_data_storage::StorageContext);

    let info = MemInfo {
        block_size_bytes: storage_context.block_size_bytes() as u32,
        total_blocks: storage_context.total_blocks(),
        used_blocks: storage_context.used_blocks(),
        freq_multiplier: crate::config::FREQ_MULTIPLIER,
    };

    match serde_json::to_string_pretty(&info) {
        Ok(s) => store_block_data(s, dest, size, offset),
        Err(e) => defmt::error!(
            "Failed to serialise flash info: {}",
            defmt::Display2Format(&e)
        ),
    }
}

pub(crate) unsafe extern "C" fn flash_read(dest: *mut u8, size: i32, offset: u32, userdata: usize) {
    if size <= 0 {
        return;
    }

    if userdata == 0 {
        return;
    }

    let read_user_data = &*(userdata as *const FlashReadUserData);
    let storage_context = &*read_user_data.storage_context;

    if storage_context.is_erase_in_progress() {
        defmt::error!("Read blocked: storage erase is in progress");
        return;
    }

    let is_used_view = read_user_data.used_view;
    let limit = if is_used_view {
        storage_context.used_size_bytes()
    } else {
        storage_context.raw_size_bytes()
    };

    let offset = offset as usize;
    if offset >= limit {
        defmt::error!(
            "Read blocked: offset {:#x} is out of range for (limit {:#x})",
            offset,
            limit
        );
        return;
    }

    // Fallback: copy data into the BOT buffer the normal way.
    let out = core::slice::from_raw_parts_mut(dest, size as usize);
    let readable = core::cmp::min(out.len(), limit - offset);
    let target = &mut out[..readable];
    let _ = storage_context.read_range(offset, target);
}
