use stm32l4xx_hal::{
    dma::{dma1, Event},
    pac::DMA1,
};

pub trait FreqmeterDmaChannelExt {
    fn configure_tim_up(&mut self, memory_addr: u32, peripheral_addr: u32);
    fn accept_isr(&self);
}

macro_rules! impl_freqmeter_dma_channel {
    ($channel:ty, $cselr_fn:ident, $map:ident, $ccr:ident, $cgif_fn:ident) => {
        impl FreqmeterDmaChannelExt for $channel {
            fn configure_tim_up(&mut self, memory_addr: u32, peripheral_addr: u32) {
                self.stop();
                self.set_memory_address(memory_addr, false);
                self.set_peripheral_address(peripheral_addr, false);
                self.set_transfer_length(1);

                let dma = unsafe { &*DMA1::ptr() };

                dma.cselr.modify(|_, w| w.$cselr_fn().$map());
                dma.$ccr.modify(|_, w| {
                    w.pl()
                        .very_high()
                        .msize()
                        .bits16()
                        .psize()
                        .bits16()
                        .circ()
                        .set_bit()
                        .dir()
                        .from_peripheral()
                        .teie()
                        .enabled()
                        .htie()
                        .disabled()
                });

                self.listen(Event::TransferComplete);
            }

            fn accept_isr(&self) {
                unsafe {
                    (*DMA1::ptr()).ifcr.write(|w| w.$cgif_fn().set_bit());
                }
            }
        }
    };
}

impl_freqmeter_dma_channel!(dma1::C6, c6s, map7, ccr6, cgif6);
impl_freqmeter_dma_channel!(dma1::C2, c2s, map4, ccr2, cgif2);
