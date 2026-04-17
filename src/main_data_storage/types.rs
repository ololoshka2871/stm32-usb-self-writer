use super::{geometry::StorageGeometry, meta::StorageMetaHandle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageMode {
    Recorder,
    Usb,
}

pub type StorageReadRangeFn = fn(usize, usize, &mut [u8]) -> Result<(), StorageError>;
pub type StorageReaderDropFn = fn(usize);

fn no_reader_read(
    _reader_ctx: usize,
    _global_offset: usize,
    _dest: &mut [u8],
) -> Result<(), StorageError> {
    Err(StorageError::NotReady)
}

fn no_reader_drop(_reader_ctx: usize) {}

pub struct StorageContext {
    mode: StorageMode,
    geometry: StorageGeometry,
    meta: Option<StorageMetaHandle>,
    initial_used_blocks: u32,
    reader_ctx: usize,
    read_range_fn: StorageReadRangeFn,
    reader_drop_fn: StorageReaderDropFn,
}

impl StorageContext {
    pub fn new_without_reader(mode: StorageMode, geometry: StorageGeometry) -> Self {
        Self {
            mode,
            geometry,
            meta: None,
            initial_used_blocks: 0,
            reader_ctx: 0,
            read_range_fn: no_reader_read,
            reader_drop_fn: no_reader_drop,
        }
    }

    pub fn new_with_reader(
        mode: StorageMode,
        geometry: StorageGeometry,
        initial_used_blocks: u32,
        reader_ctx: usize,
        read_range_fn: StorageReadRangeFn,
        reader_drop_fn: StorageReaderDropFn,
    ) -> Self {
        Self {
            mode,
            geometry,
            meta: None,
            initial_used_blocks,
            reader_ctx,
            read_range_fn,
            reader_drop_fn,
        }
    }

    #[inline]
    pub fn mode(&self) -> StorageMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: StorageMode) {
        self.mode = mode;
    }

    #[inline]
    pub fn geometry(&self) -> StorageGeometry {
        self.geometry
    }

    pub fn meta_handle(&mut self) -> StorageMetaHandle {
        if let Some(meta) = self.meta {
            return meta;
        }

        let meta = StorageMetaHandle::new(self.geometry);
        meta.set_used_blocks(self.initial_used_blocks);
        self.meta = Some(meta);
        meta
    }

    pub fn meta_handle_if_present(&self) -> Option<StorageMetaHandle> {
        self.meta
    }

    pub fn read_range(&self, global_offset: usize, dest: &mut [u8]) -> Result<(), StorageError> {
        (self.read_range_fn)(self.reader_ctx, global_offset, dest)
    }

    pub fn block_size_bytes(&self) -> usize {
        self.geometry.block_size_bytes as usize
    }

    pub fn total_blocks(&self) -> u32 {
        self.geometry.total_blocks()
    }

    pub fn used_blocks(&self) -> u32 {
        self.meta_handle_if_present()
            .map(|meta| meta.used_blocks())
            .unwrap_or(self.initial_used_blocks)
    }

    pub fn raw_size_bytes(&self) -> usize {
        self.block_size_bytes().saturating_mul(self.total_blocks() as usize)
    }

    pub fn used_size_bytes(&self) -> usize {
        self.block_size_bytes()
            .saturating_mul(self.used_blocks() as usize)
    }

    pub fn is_erase_in_progress(&self) -> bool {
        self.meta_handle_if_present()
            .map(|meta| meta.erase_in_progress())
            .unwrap_or(false)
    }
}

impl Drop for StorageContext {
    fn drop(&mut self) {
        (self.reader_drop_fn)(self.reader_ctx);
        self.reader_ctx = 0;
        self.read_range_fn = no_reader_read;
        self.reader_drop_fn = no_reader_drop;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    Busy,
    NotReady,
    InvalidAddress,
    Internal,
}
