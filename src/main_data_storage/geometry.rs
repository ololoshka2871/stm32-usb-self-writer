#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashBanks {
    One,
    Two,
}

impl FlashBanks {
    #[inline]
    pub const fn count(self) -> u32 {
        match self {
            FlashBanks::One => 1,
            FlashBanks::Two => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageGeometry {
    pub block_size_bytes: u32,
    pub write_granularity_bytes: u32,
    pub blocks_per_bank: u32,
    pub banks: FlashBanks,
}

impl StorageGeometry {
    #[inline]
    pub const fn total_blocks(self) -> u32 {
        self.blocks_per_bank * self.banks.count()
    }

    #[inline]
    pub const fn total_bytes(self) -> usize {
        self.total_blocks() as usize * self.block_size_bytes as usize
    }

    #[inline]
    pub const fn is_dual(self) -> bool {
        matches!(self.banks, FlashBanks::Two)
    }

    pub const fn validate(self) -> Result<(), GeometryError> {
        if self.block_size_bytes == 0 {
            return Err(GeometryError::ZeroBlockSize);
        }
        if self.write_granularity_bytes == 0 {
            return Err(GeometryError::ZeroWriteGranularity);
        }
        if self.blocks_per_bank == 0 {
            return Err(GeometryError::ZeroBlocksPerBank);
        }
        if self.block_size_bytes % self.write_granularity_bytes != 0 {
            return Err(GeometryError::BlockNotAligned);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryError {
    ZeroBlockSize,
    ZeroWriteGranularity,
    ZeroBlocksPerBank,
    BlockNotAligned,
}
