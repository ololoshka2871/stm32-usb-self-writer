use super::geometry::{FlashBanks, StorageGeometry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockAddress {
    pub bank_index: u8,
    pub local_block_index: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OffsetAddress {
    pub bank_index: u8,
    pub local_byte_offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum MapperError {
    BlockOutOfRange,
    OffsetOutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockMapper {
    geometry: StorageGeometry,
}

impl BlockMapper {
    pub const fn new(geometry: StorageGeometry) -> Self {
        Self { geometry }
    }

    #[inline]
    pub const fn geometry(self) -> StorageGeometry {
        self.geometry
    }

    #[inline]
    pub const fn total_blocks(self) -> u32 {
        self.geometry.total_blocks()
    }

    #[inline]
    pub const fn raw_size_bytes(self) -> usize {
        self.geometry.total_bytes()
    }

    pub const fn used_size_bytes(self, used_blocks: u32) -> usize {
        let clamped = if used_blocks > self.total_blocks() {
            self.total_blocks()
        } else {
            used_blocks
        };
        clamped as usize * self.geometry.block_size_bytes as usize
    }

    pub const fn map_block(self, global_block_index: u32) -> Result<BlockAddress, MapperError> {
        if global_block_index >= self.total_blocks() {
            return Err(MapperError::BlockOutOfRange);
        }

        match self.geometry.banks {
            FlashBanks::One => Ok(BlockAddress {
                bank_index: 0,
                local_block_index: global_block_index,
            }),
            FlashBanks::Two => Ok(BlockAddress {
                bank_index: (global_block_index % 2) as u8,
                local_block_index: global_block_index / 2,
            }),
        }
    }

    pub fn map_offset(self, global_offset: usize) -> Result<OffsetAddress, MapperError> {
        if global_offset >= self.raw_size_bytes() {
            return Err(MapperError::OffsetOutOfRange);
        }

        let block_size = self.geometry.block_size_bytes as usize;
        let global_block_index = (global_offset / block_size) as u32;
        let in_block_offset = global_offset % block_size;
        let block = self.map_block(global_block_index)?;

        Ok(OffsetAddress {
            bank_index: block.bank_index,
            local_byte_offset: block.local_block_index as usize * block_size + in_block_offset,
        })
    }
}
