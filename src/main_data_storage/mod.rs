pub mod api;
pub mod backend;
pub mod core;
pub mod geometry;
pub mod mapper;
pub mod meta;
pub mod types;

pub use api::{Storage, StorageDriver, StorageInfo};
pub use backend::{NullBackend, StorageBackend};
pub use core::StorageCore;
pub use geometry::{FlashBanks, GeometryError, StorageGeometry};
pub use mapper::{BlockAddress, BlockMapper, MapperError, OffsetAddress};
pub use meta::{StorageMetaError, StorageMetaHandle};
pub use types::{StorageContext, StorageEraseHandle, StorageError, StorageMode};
