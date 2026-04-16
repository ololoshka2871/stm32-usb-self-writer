pub mod api;
pub mod backend;
pub mod core;
pub mod geometry;
pub mod mapper;
pub mod meta;
pub mod runtime;
pub mod types;

pub use api::{Storage, StorageDriver, StorageInfo};
pub use backend::{NullBackend, StorageBackend};
pub use core::StorageCore;
pub use geometry::{FlashBanks, GeometryError, StorageGeometry};
pub use mapper::{BlockAddress, BlockMapper, MapperError, OffsetAddress};
pub use meta::{StorageMetaError, StorageMetaHandle};
pub use runtime::{
	block_size_bytes, has_read_range_fn, has_storage_meta, install_meta_handle,
	install_read_range_fn, is_erase_in_progress, raw_size_bytes, read_range,
	refresh_runtime_snapshot, request_erase, total_blocks, used_blocks, used_size_bytes,
	with_meta_handle,
};
pub use types::{StorageContext, StorageError, StorageMode};
