use alloc::{boxed::Box, sync::Arc};

pub trait StorageInfo {
    fn flash_page_size(&self) -> usize;
    fn flash_size(&self) -> usize;
    fn used_pages(&self) -> usize;
}

pub trait StorageDriver {
    fn make_info_accessor(&self, driver: Arc<dyn StorageDriver>) -> Box<dyn StorageInfo>;
}

pub struct Storage {
    driver: Arc<dyn StorageDriver>,
}

impl Storage {
    pub fn new(driver: impl StorageDriver + 'static) -> Self {
        Self {
            driver: Arc::new(driver),
        }
    }

    pub fn make_info_accessor(&self) -> Box<dyn StorageInfo> {
        self.driver.make_info_accessor(Arc::clone(&self.driver))
    }
}
