use super::{
    mapper::BlockMapper, meta::StorageMetaHandle, GeometryError, StorageBackend, StorageContext,
    StorageError, StorageMode,
};

pub struct StorageCore<B>
where
    B: StorageBackend,
{
    backend: B,
    mapper: BlockMapper,
    context: StorageContext,
}

impl<B> StorageCore<B>
where
    B: StorageBackend,
{
    pub fn new(mode: StorageMode, backend: B) -> Result<Self, GeometryError> {
        let geometry = backend.geometry();
        geometry.validate()?;

        let mapper = BlockMapper::new(geometry);
        let mut context = StorageContext::new_without_reader(mode, geometry);
        let _ = context.meta_handle();

        Ok(Self {
            backend,
            mapper,
            context,
        })
    }

    #[inline]
    pub fn context(&self) -> &StorageContext {
        &self.context
    }

    #[inline]
    pub fn mapper(&self) -> BlockMapper {
        self.mapper
    }

    #[inline]
    pub fn meta(&mut self) -> StorageMetaHandle {
        self.context.meta_handle()
    }

    pub fn set_mode(&mut self, mode: StorageMode) {
        self.context.set_mode(mode);
    }

    pub fn startup_scan(&mut self) -> Result<u32, StorageError> {
        let used = self.backend.scan_used_blocks()?;
        let meta = self.context.meta_handle();
        meta.set_used_blocks(used);
        Ok(meta.used_blocks())
    }

    pub fn write_next_block(&mut self, data: &[u8]) -> Result<u32, StorageError> {
        if self.context.mode() != StorageMode::Recorder {
            return Err(StorageError::NotReady);
        }
        let geometry = self.context.geometry();
        if data.len() != geometry.block_size_bytes as usize {
            return Err(StorageError::InvalidAddress);
        }

        let meta = self.context.meta_handle();
        let next_block = meta.used_blocks();
        if next_block >= geometry.total_blocks() {
            return Err(StorageError::NotReady);
        }

        meta.set_busy(true);
        self.backend.set_sleep(false)?;
        let result = self.backend.write_block(next_block, data);
        let _ = self.backend.set_sleep(true);
        meta.set_busy(false);

        result.map(|_| {
            meta.increment_used_blocks();
            next_block
        })
    }

    pub fn read_range(&mut self, offset: usize, dest: &mut [u8]) -> Result<(), StorageError> {
        if self.context.meta_handle().erase_in_progress() {
            return Err(StorageError::NotReady);
        }
        self.backend.read_range(offset, dest)
    }

    pub fn request_erase(&self) -> Result<(), super::StorageMetaError> {
        self.context
            .meta_handle_if_present()
            .ok_or(super::StorageMetaError::EraseAlreadyInProgress)?
            .request_erase()
    }

    pub fn process_pending_erase(&mut self) -> Result<bool, StorageError> {
        let meta = self.context.meta_handle();
        if !meta.take_erase_request() {
            return Ok(false);
        }

        meta.set_erase_in_progress(true);
        meta.set_busy(true);
        let erase_result = self.backend.erase_all();
        meta.set_busy(false);
        meta.set_erase_in_progress(false);

        match erase_result {
            Ok(()) => {
                meta.reset_used_blocks();
                Ok(true)
            }
            Err(e) => Err(e),
        }
    }
}
