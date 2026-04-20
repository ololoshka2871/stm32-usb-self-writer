use alloc::boxed::Box;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::geometry::StorageGeometry;

#[derive(Debug)]
struct StorageMetaState {
    block_size_bytes: u32,
    total_blocks: u32,
    used_blocks: AtomicU32,
    busy: AtomicBool,
    erase_in_progress: AtomicBool,
    erase_requested: AtomicBool,
}

#[derive(Clone, Copy)]
pub struct StorageMetaHandle {
    state: NonNull<StorageMetaState>,
}

impl StorageMetaHandle {
    pub fn new(geometry: StorageGeometry) -> Self {
        let state = StorageMetaState {
            block_size_bytes: geometry.block_size_bytes,
            total_blocks: geometry.total_blocks(),
            used_blocks: AtomicU32::new(0),
            busy: AtomicBool::new(false),
            erase_in_progress: AtomicBool::new(false),
            erase_requested: AtomicBool::new(false),
        };

        let leaked = Box::leak(Box::new(state));
        Self {
            state: NonNull::from(leaked),
        }
    }

    #[inline]
    fn state(self) -> &'static StorageMetaState {
        unsafe { self.state.as_ref() }
    }

    #[inline]
    pub fn block_size_bytes(self) -> u32 {
        self.state().block_size_bytes
    }

    #[inline]
    pub fn total_blocks(self) -> u32 {
        self.state().total_blocks
    }

    #[inline]
    pub fn used_blocks(self) -> u32 {
        self.state().used_blocks.load(Ordering::Acquire)
    }

    pub fn set_used_blocks(self, used: u32) {
        let clamped = if used > self.total_blocks() {
            self.total_blocks()
        } else {
            used
        };
        self.state().used_blocks.store(clamped, Ordering::Release);
    }

    pub fn increment_used_blocks(self) {
        let current = self.used_blocks();
        if current < self.total_blocks() {
            self.set_used_blocks(current + 1);
        }
    }

    pub fn reset_used_blocks(self) {
        self.set_used_blocks(0);
    }

    #[inline]
    pub fn is_busy(self) -> bool {
        self.state().busy.load(Ordering::Acquire)
    }

    pub fn set_busy(self, busy: bool) {
        self.state().busy.store(busy, Ordering::Release);
    }

    #[inline]
    pub fn erase_in_progress(self) -> bool {
        self.state().erase_in_progress.load(Ordering::Acquire)
    }

    pub fn set_erase_in_progress(self, in_progress: bool) {
        self.state()
            .erase_in_progress
            .store(in_progress, Ordering::Release);
    }

    pub fn request_erase(self) -> Result<(), StorageMetaError> {
        if self.erase_in_progress() {
            return Err(StorageMetaError::EraseAlreadyInProgress);
        }

        let exchanged = self
            .state()
            .erase_requested
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire);

        match exchanged {
            Ok(_) => Ok(()),
            Err(_) => Err(StorageMetaError::EraseAlreadyRequested),
        }
    }

    #[inline]
    pub fn is_erase_requested(self) -> bool {
        self.state().erase_requested.load(Ordering::Acquire)
    }

    pub fn take_erase_request(self) -> bool {
        self.state()
            .erase_requested
            .swap(false, Ordering::AcqRel)
    }

    pub fn clear_erase_request(self) {
        self.state().erase_requested.store(false, Ordering::Release);
    }

}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageMetaError {
    EraseAlreadyRequested,
    EraseAlreadyInProgress,
}

unsafe impl Send for StorageMetaHandle {}
unsafe impl Sync for StorageMetaHandle {}
