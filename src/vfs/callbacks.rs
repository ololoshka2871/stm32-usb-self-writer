#![allow(dead_code, unsafe_op_in_unsafe_fn)]

use core::cell::RefCell;

use alloc::{boxed::Box, string::String, sync::Arc};

use heatshrink_rust::{CompressedData, decoder::HeatshrinkDecoder};

use super::StaticBinData;

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

pub (crate) unsafe extern "C" fn null_read(_dest: *mut u8, _size: i32, _offset: u32, _userdata: usize) {}

pub(crate) const STORAGE_VIEW_RAW: usize = 1;
pub(crate) const STORAGE_VIEW_USED: usize = 2;

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
    _userdata: usize,
) {
    use serde::Serialize;

    #[allow(non_snake_case)]
    #[derive(Serialize)]
    struct MemInfo {
        BlockSizeBytes: u32,
        TotalBlocks: u32,
        UsedBlocks: u32,
        EraseInProgress: bool,
    }

    crate::main_data_storage::refresh_runtime_snapshot();

    let info = MemInfo {
        BlockSizeBytes: crate::main_data_storage::block_size_bytes() as u32,
        TotalBlocks: crate::main_data_storage::total_blocks(),
        UsedBlocks: crate::main_data_storage::used_blocks(),
        EraseInProgress: crate::main_data_storage::is_erase_in_progress(),
    };

    match serde_json::to_string_pretty(&info) {
        Ok(s) => store_block_data(s, dest, size, offset),
        Err(e) => defmt::error!(
            "Failed to serialise flash info: {}",
            defmt::Display2Format(&e)
        ),
    }
}
//
//pub(crate) unsafe extern "C" fn master_read(
//    dest: *mut u8,
//    _size: i32,
//    _offset: u32,
//    userdata: usize,
//) {
//    let boxed = alloc::boxed::Box::from_raw(
//        userdata as *mut crate::sensors::freqmeter::master_counter::MasterTimerInfo,
//    );
//
//    let s = alloc::format!("0x{:08X}", boxed.value().0);
//    core::ptr::copy_nonoverlapping(s.as_ptr(), dest, s.len());
//
//    core::mem::forget(boxed);
//}

pub(crate) unsafe extern "C" fn flash_read(dest: *mut u8, size: i32, offset: u32, userdata: usize) {
    if size <= 0 {
        return;
    }

    let out = core::slice::from_raw_parts_mut(dest, size as usize);
    out.fill(0);

    crate::main_data_storage::refresh_runtime_snapshot();
    if crate::main_data_storage::is_erase_in_progress() {
        return;
    }

    let is_used_view = userdata == STORAGE_VIEW_USED;
    let limit = if is_used_view {
        crate::main_data_storage::used_size_bytes()
    } else {
        crate::main_data_storage::raw_size_bytes()
    };

    let offset = offset as usize;
    if offset >= limit {
        return;
    }

    let readable = core::cmp::min(out.len(), limit - offset);
    let target = &mut out[..readable];

    let _ = crate::main_data_storage::read_range(offset, target);
}
