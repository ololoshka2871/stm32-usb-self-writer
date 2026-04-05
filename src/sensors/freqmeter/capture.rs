#[derive(Clone, Copy, defmt::Format)]
pub struct Capture {
    pub target: u16,
    pub dma_value: u32,
}

impl Capture {
    pub fn wrapping_sub(&self, other: Capture) -> u32 {
        self.dma_value.wrapping_sub(other.dma_value)
    }
}
