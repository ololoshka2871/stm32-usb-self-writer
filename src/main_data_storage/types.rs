use super::{geometry::StorageGeometry, meta::StorageMetaHandle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageMode {
    Recorder,
    Usb,
}

#[derive(Clone, Copy)]
pub struct StorageContext {
    pub mode: StorageMode,
    pub geometry: StorageGeometry,
    pub meta: StorageMetaHandle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    Busy,
    NotReady,
    InvalidAddress,
    Internal,
}
