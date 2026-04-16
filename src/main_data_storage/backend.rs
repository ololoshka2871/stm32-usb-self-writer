use super::{StorageError, StorageGeometry};

pub trait StorageBackend {
    fn geometry(&self) -> StorageGeometry;

    fn scan_used_blocks(&mut self) -> Result<u32, StorageError>;

    fn read_range(&mut self, global_offset: usize, dest: &mut [u8]) -> Result<(), StorageError>;

    fn write_block(&mut self, global_block_index: u32, data: &[u8]) -> Result<(), StorageError>;

    fn erase_all(&mut self) -> Result<(), StorageError>;

    fn set_sleep(&mut self, enabled: bool) -> Result<(), StorageError>;

    fn supports_memory_mapped(&self) -> bool {
        false
    }
}

pub struct NullBackend {
    geometry: StorageGeometry,
}

impl NullBackend {
    pub const fn new(geometry: StorageGeometry) -> Self {
        Self { geometry }
    }
}

impl StorageBackend for NullBackend {
    fn geometry(&self) -> StorageGeometry {
        self.geometry
    }

    fn scan_used_blocks(&mut self) -> Result<u32, StorageError> {
        Ok(0)
    }

    fn read_range(&mut self, _global_offset: usize, _dest: &mut [u8]) -> Result<(), StorageError> {
        Err(StorageError::NotReady)
    }

    fn write_block(&mut self, _global_block_index: u32, _data: &[u8]) -> Result<(), StorageError> {
        Err(StorageError::NotReady)
    }

    fn erase_all(&mut self) -> Result<(), StorageError> {
        Ok(())
    }

    fn set_sleep(&mut self, _enabled: bool) -> Result<(), StorageError> {
        Ok(())
    }
}
