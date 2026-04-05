mod capture;
mod capturer;
mod input_counter;

pub mod dma_traits;
pub mod freqmeter;
pub mod master_counter;
pub mod tim_input_config_helper;

pub use capture::Capture;
pub use capturer::Capturer;
pub use freqmeter::Freqmeter;
pub use input_counter::{InputCounter, TimerInpitCounterExt};
pub use master_counter::*;

#[macro_export]
macro_rules! freqmeter_dma_interrupt {
    (
        buffer: $buffer:expr,
        cature_tx: $cature_tx:expr,
        target_rx: $target_rx:expr,
        capturerer: $capturerer:expr,
        transfer: $transfer:expr,
        cgifX: $cgifX:ident
    ) => {{
        let capture = $capturerer.lock(|capturerer| capturerer.capture($buffer));

        $cature_tx.try_send(capture).ok();

        if let Ok(new_tgt) = $target_rx.try_recv() {
            $capturerer.lock(|capturerer| {
                capturerer.stop();
                capturerer.start(new_tgt);
            });
        }

        $transfer.lock(|transfer| {
            transfer.stop();
            transfer.ifcr().write(|w| w.$cgifX().set_bit()); // Clear DMA interrupt flag
            transfer.start();
        });
    }};
}

#[macro_export]
macro_rules! build_freqmeter {
    (
        input_timer=$input_timer:expr,
        dma_channel=$dma_channel:expr,
        master_timer=$master_timer:expr,
        master_type=$master_type:ty,
        dp=$dp:ident,
        stop_reg=$stop_reg:ident,
        stop_bit=$stop_bit:ident
    ) => {{
        use stm32_usb_self_writer::sensors::freqmeter::{
            dma_traits::PeriAddress, Capture, Freqmeter,
        };
        use stm32l4xx_hal::dma::Event;

        let mut capturer = $master_timer.make_capturer($input_timer);

        #[cfg(debug_assertions)]
        $dp.DBGMCU.$stop_reg.modify(|_, w| w.$stop_bit().set_bit());

        let buffer = unsafe { cortex_m::singleton!(: $master_type = 0).unwrap_unchecked() };

        // FIXME: STM32L4 use Advanced DMA, like F4
        //let mut dma_transfer = $dma_channel;
        //dma_transfer.set_memory_address(buffer as *const _ as u32, false);
        //dma_transfer.set_peripheral_address(capturer.address(), false);
        //dma_transfer.set_transfer_length(1);
        //dma_transfer.ch().cr.modify(|_, w|
        //    // по неустановленой причине, хотя счетчик нормально считает все 32 бита,
        //    // но DMA не хочет копировать все 32 бита, только младшие 16
        //    w
        //        .pl().high()
        //        .msize().bits32()
        //        .psize().bits16()
        //        .dir().from_peripheral()
        //        .circ().set_bit()
        //);
        //dma_transfer.listen(Event::TransferComplete);
        //dma_transfer.start();

        capturer.start(config::INITIAL_FREQMETER_TARGET);

        let freqmeter = Freqmeter::new();

        (
            freqmeter,
            /*dma_transfer,*/
            capturer,
            buffer,
            rtic_sync::make_channel!(Capture, 1),
            rtic_sync::make_channel!(u16, 1),
        )
    }};
}
