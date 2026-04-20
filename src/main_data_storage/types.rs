use super::{geometry::StorageGeometry, meta::StorageMetaHandle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageMode {
    Recorder,
    Usb,
}

pub type StorageReadRangeFn = fn(usize, usize, &mut [u8]) -> Result<(), StorageError>;
pub type StorageWriteBlockFn = fn(usize, u32, &[u8]) -> Result<(), StorageError>;
pub type StorageReaderDropFn = fn(usize);
pub type StorageEraseFn = fn(usize) -> Result<(), StorageError>;

#[derive(Clone, Copy)]
pub struct StorageEraseHandle {
    meta: StorageMetaHandle,
    reader_ctx: usize,
    erase_fn: StorageEraseFn,
}

impl StorageEraseHandle {
    pub fn has_pending_request(self) -> bool {
        self.meta.is_erase_requested()
    }

    pub fn is_in_progress(self) -> bool {
        self.meta.erase_in_progress()
    }

    pub fn process_pending_erase(self) -> Result<bool, StorageError> {
        if !self.meta.take_erase_request() {
            return Ok(false);
        }

        self.meta.set_erase_in_progress(true);
        self.meta.set_busy(true);
        let erase_result = (self.erase_fn)(self.reader_ctx);
        self.meta.set_busy(false);
        self.meta.set_erase_in_progress(false);

        match erase_result {
            Ok(()) => {
                self.meta.reset_used_blocks();
                Ok(true)
            }
            Err(e) => Err(e),
        }
    }
}

fn no_reader_read(
    _reader_ctx: usize,
    _global_offset: usize,
    _dest: &mut [u8],
) -> Result<(), StorageError> {
    Err(StorageError::NotReady)
}

fn no_reader_drop(_reader_ctx: usize) {}

fn no_write_block(_reader_ctx: usize, _global_block_index: u32, _data: &[u8]) -> Result<(), StorageError> {
    Err(StorageError::NotReady)
}

fn no_erase(_reader_ctx: usize) -> Result<(), StorageError> {
    Err(StorageError::NotReady)
}

pub struct StorageContext {
    mode: StorageMode,
    geometry: StorageGeometry,
    meta: Option<StorageMetaHandle>,
    initial_used_blocks: u32,
    reader_ctx: usize,
    read_range_fn: StorageReadRangeFn,
    write_block_fn: StorageWriteBlockFn,
    reader_drop_fn: StorageReaderDropFn,
    erase_fn: StorageEraseFn,
}

impl StorageContext {
    pub fn new_empty(mode: StorageMode, geometry: StorageGeometry) -> Self {
        Self {
            mode,
            geometry,
            meta: None,
            initial_used_blocks: 0,
            reader_ctx: 0,
            read_range_fn: no_reader_read,
            write_block_fn: no_write_block,
            reader_drop_fn: no_reader_drop,
            erase_fn: no_erase,
        }
    }

    pub fn new(
        mode: StorageMode,
        geometry: StorageGeometry,
        initial_used_blocks: u32,
        reader_ctx: usize,
        read_range_fn: StorageReadRangeFn,
        write_block_fn: StorageWriteBlockFn,
        reader_drop_fn: StorageReaderDropFn,
        erase_fn: StorageEraseFn,
    ) -> Self {
        Self {
            mode,
            geometry,
            meta: None,
            initial_used_blocks,
            reader_ctx,
            read_range_fn,
            write_block_fn,
            reader_drop_fn,
            erase_fn,
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

    pub fn write_next_block(&mut self, data: &[u8]) -> Result<u32, StorageError> {
        if self.mode() != StorageMode::Recorder {
            return Err(StorageError::NotReady);
        }

        let geometry = self.geometry();
        if data.len() != geometry.block_size_bytes as usize {
            return Err(StorageError::InvalidAddress);
        }

        let meta = self.meta_handle();
        let next_block = meta.used_blocks();
        if next_block >= geometry.total_blocks() {
            return Err(StorageError::NotReady);
        }

        meta.set_busy(true);
        let result = (self.write_block_fn)(self.reader_ctx, next_block, data);
        meta.set_busy(false);

        result.map(|_| {
            meta.increment_used_blocks();
            next_block
        })
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

    pub fn process_pending_erase(&mut self) -> Result<bool, StorageError> {
        self.erase_handle().process_pending_erase()
    }

    pub fn erase_handle(&mut self) -> StorageEraseHandle {
        StorageEraseHandle {
            meta: self.meta_handle(),
            reader_ctx: self.reader_ctx,
            erase_fn: self.erase_fn,
        }
    }
}

impl Drop for StorageContext {
    fn drop(&mut self) {
        (self.reader_drop_fn)(self.reader_ctx);
        self.reader_ctx = 0;
        self.read_range_fn = no_reader_read;
        self.write_block_fn = no_write_block;
        self.reader_drop_fn = no_reader_drop;
        self.erase_fn = no_erase;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    Busy,
    NotReady,
    InvalidAddress,
    Internal,
}
