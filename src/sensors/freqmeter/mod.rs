pub mod capture;
pub mod capturerer;
pub mod dma_traits;
pub mod freqmeter;
pub mod input_counter;
pub mod master_counter;
pub mod tim_input_config_helper;

pub use capture::Capture;
pub use freqmeter::Freqmeter;
pub use master_counter::*;
pub use scaffold::FreqmetersScaffold;

#[macro_export]
macro_rules! freqmeter_dma_interrupt {
    (buffer: $buffer:expr, cature_tx: $cature_tx:expr, target_rx: $target_rx:expr,
    capturerer: $capturerer:expr, transfer: $transfer:expr, cgifX: $cgifX:ident) => {{
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
    (input_timer=$input_timer:expr,
     stop_reg=$stop_reg:ident,
     stop_bit=$stop_bit:ident,
     dma_channel=$dma_channel:expr,
     master_timer=$master_timer:expr,
     dp=$dp:ident) => {{
        let mut capturer = $master_timer.make_capturer($input_timer);

        #[cfg(debug_assertions)]
        $dp.DBGMCU.$stop_reg.modify(|_, w| w.$stop_bit().set_bit());

        let buffer = unsafe { cortex_m::singleton!(: support::m_tim2::Type = 0).unwrap_unchecked() };

        let mut dma_transfer = $dma_channel;
        dma_transfer.set_memory_address(buffer as *const _ as u32, false);
        dma_transfer.set_peripheral_address(capturer.address(), false);
        dma_transfer.set_transfer_length(1);
        dma_transfer.ch().cr.modify(|_, w|
            // по неустановленой причине, хотя счетчик нормально считает все 32 бита,
            // но DMA не хочет копировать все 32 бита, только младшие 16
            w
                .pl().high()
                .msize().bits32()
                .psize().bits16()
                .dir().from_peripheral()
                .circ().set_bit()
        );
        dma_transfer.listen(Event::TransferComplete);
        dma_transfer.start();

        capturer.start(config::FREQMETER_INITIAL_TARGET);

        let freqmeter = Freqmeter::<{ config::SYST_CLOCK_HZ }>::new();

        (
            freqmeter,
            dma_transfer,
            capturer,
            buffer,
            rtic_sync::make_channel!(Capture, 1),
            rtic_sync::make_channel!(u16, 1),
        )
    }};
}
