use alloc::{boxed::Box, vec::Vec};

pub trait Stream<E> {
    fn read(&mut self, buf: &mut [u8]) -> Result<(), E>;
    fn read_all(&mut self) -> Result<Vec<u8>, E>;
}

#[async_trait::async_trait]
pub trait AsyncStream<E> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<(), E>;
    async fn read_size(&mut self, size: usize) -> Result<Vec<u8>, E>;
}
