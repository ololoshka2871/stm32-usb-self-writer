use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::StorageMetaHandle;

static META_INSTALLED: AtomicBool = AtomicBool::new(false);
static BLOCK_SIZE_BYTES: AtomicU32 = AtomicU32::new(0);
static TOTAL_BLOCKS: AtomicU32 = AtomicU32::new(0);
static USED_BLOCKS: AtomicU32 = AtomicU32::new(0);
static ERASE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

static mut META_HANDLE: Option<StorageMetaHandle> = None;
static mut READ_RANGE_FN: Option<fn(usize, &mut [u8]) -> Result<(), super::StorageError>> = None;
static READ_FN_INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn install_meta_handle(handle: StorageMetaHandle) {
    BLOCK_SIZE_BYTES.store(handle.block_size_bytes(), Ordering::Release);
    TOTAL_BLOCKS.store(handle.total_blocks(), Ordering::Release);
    USED_BLOCKS.store(handle.used_blocks(), Ordering::Release);
    ERASE_IN_PROGRESS.store(handle.erase_in_progress(), Ordering::Release);

    unsafe {
        META_HANDLE = Some(handle);
    }
    META_INSTALLED.store(true, Ordering::Release);
}

pub fn with_meta_handle<T>(f: impl FnOnce(StorageMetaHandle) -> T) -> Option<T> {
    if !META_INSTALLED.load(Ordering::Acquire) {
        return None;
    }

    let handle = unsafe { META_HANDLE };
    handle.map(f)
}

pub fn install_read_range_fn(
    read_fn: fn(usize, &mut [u8]) -> Result<(), super::StorageError>,
) {
    unsafe {
        READ_RANGE_FN = Some(read_fn);
    }
    READ_FN_INSTALLED.store(true, Ordering::Release);
}

pub fn has_read_range_fn() -> bool {
    READ_FN_INSTALLED.load(Ordering::Acquire)
}

pub fn read_range(global_offset: usize, dest: &mut [u8]) -> Result<(), super::StorageError> {
    if !has_read_range_fn() {
        return Err(super::StorageError::NotReady);
    }

    let f = unsafe { READ_RANGE_FN };
    if let Some(read_fn) = f {
        read_fn(global_offset, dest)
    } else {
        Err(super::StorageError::NotReady)
    }
}

pub fn refresh_runtime_snapshot() {
    let _ = with_meta_handle(|h| {
        USED_BLOCKS.store(h.used_blocks(), Ordering::Release);
        ERASE_IN_PROGRESS.store(h.erase_in_progress(), Ordering::Release);
    });
}

pub fn block_size_bytes() -> usize {
    BLOCK_SIZE_BYTES.load(Ordering::Acquire) as usize
}

pub fn total_blocks() -> u32 {
    TOTAL_BLOCKS.load(Ordering::Acquire)
}

pub fn used_blocks() -> u32 {
    USED_BLOCKS.load(Ordering::Acquire)
}

pub fn raw_size_bytes() -> usize {
    block_size_bytes().saturating_mul(total_blocks() as usize)
}

pub fn used_size_bytes() -> usize {
    block_size_bytes().saturating_mul(used_blocks() as usize)
}

pub fn is_erase_in_progress() -> bool {
    ERASE_IN_PROGRESS.load(Ordering::Acquire)
}

pub fn request_erase() -> Result<(), super::StorageMetaError> {
    with_meta_handle(|h| h.request_erase()).unwrap_or(Err(super::StorageMetaError::EraseAlreadyInProgress))
}

pub fn has_storage_meta() -> bool {
    META_INSTALLED.load(Ordering::Acquire)
}
