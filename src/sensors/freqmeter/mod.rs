mod capture;
mod capturer;
mod input_counter;

pub mod dma_traits;
pub mod freqmeter;
pub mod freqmeter_dma_l4;
pub mod master_counter;
pub mod tim_input_config_helper;

pub use capture::Capture;
pub use capturer::Capturer;
pub use freqmeter::Freqmeter;
pub use freqmeter_dma_l4::FreqmeterDmaChannelExt;
pub use input_counter::{ExtInputType, InputCounter, TimerInpitCounterExt};
pub use master_counter::*;

#[macro_export]
macro_rules! freqmeter_dma_interrupt {
    (
        buffer: $buffer:expr,
        capture_tx: $capture_tx:expr,
        target_rx: $target_rx:expr,
        capturer: $capturer:expr,
        transfer: $transfer:expr,
        cgifX: $cgifX:ident
    ) => {{
        use stm32_usb_self_writer::sensors::freqmeter::FreqmeterDmaChannelExt;

        let buffer = $buffer;
        let capture = $capturer.lock(move |capturer| capturer.capture(buffer));

        $capture_tx.try_send(capture).ok();

        if let Ok(new_tgt) = $target_rx.try_recv() {
            $capturer.lock(|capturer| {
                capturer.stop();
                capturer.start(new_tgt);
            });
        }

        $transfer.lock(|transfer| transfer.accept_isr());
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
            freqmeter_dma_l4::FreqmeterDmaChannelExt,
            dma_traits::PeriAddress, Capture, Freqmeter,
        };

        let mut capturer = $master_timer.make_capturer($input_timer);

        #[cfg(debug_assertions)]
        $dp.DBGMCU.$stop_reg.modify(|_, w| w.$stop_bit().set_bit());

        let buffer = unsafe { cortex_m::singleton!(: $master_type = 0).unwrap_unchecked() };

        let mut dma_transfer = $dma_channel;
        dma_transfer.freqmeter_configure(buffer as *const _ as u32, capturer.address());

        capturer.start(config::INITIAL_FREQMETER_TARGET);

        let freqmeter = Freqmeter::new();

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
