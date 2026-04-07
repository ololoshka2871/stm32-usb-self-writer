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
        channel=$channel:expr,
        buffer=$buffer:expr,
        capture_tx=$capture_tx:expr,
        capturer=$capturer:expr,
        transfer=$transfer:expr,
        //target_rx=$target_rx:expr,
    ) => {{
        use stm32_usb_self_writer::sensors::freqmeter::FreqmeterDmaChannelExt;

        let buffer = $buffer;
        let capture = $capturer.lock(move |capturer| capturer.capture(buffer));

        $capture_tx.try_send(capture).ok();

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
        power_pin=$power_pin:expr,
        dp=$dp:ident,
        stop_reg=$stop_reg:ident,
        stop_bit=$stop_bit:ident
    ) => {{
        use stm32_usb_self_writer::sensors::freqmeter::{
            freqmeter_dma_l4::FreqmeterDmaChannelExt,
            dma_traits::PeriAddress, Capture, Freqmeter,
        };

        let mut capturer = $master_timer.make_capturer($input_timer);

        $dp.DBGMCU.$stop_reg.modify(|_, w| w.$stop_bit().set_bit());

        let buffer = unsafe { cortex_m::singleton!(: $master_type = 0).unwrap_unchecked() };

        let mut dma_transfer = $dma_channel;
        dma_transfer.configure_tim_up(buffer as *const _ as u32, capturer.address());

        //capturer.start(config::INITIAL_FREQMETER_TARGET);

        let freqmeter = Freqmeter::with_power_pin($power_pin);

        (
            freqmeter,
            dma_transfer,
            capturer,
            buffer,
            rtic_sync::make_channel!(Capture, 1),
        )
    }};
}

#[macro_export]
macro_rules! freqmeter {
    (
        channel=$channel:expr,
        start_event=$start_event:expr,
        base_period=$base_period:expr,
        base_period_devider=$base_period_devider:expr,
        capture_rx=$capture_rx:expr,
        freqmeter=$freqmeter:expr,
        //data_storage=$data_storage:expr,
        transfer_fin=$transfer_fin:expr,
        f_capturer=$f_capturer:expr,
        f_ref=$f_ref:expr,
    ) => {
        use stm32_usb_self_writer::sensors::freqmeter::FreqmeterDmaChannelExt;

        //let get_measure_settings = move |data_storage: &mut &mut data_storage::DataStorage| {
        //    (
        //        data_storage.holdings.get_measure_time($channel) as u64,
        //        data_storage.holdings.get_f_ref().hz(),
        //        data_storage.holdings.get_pwm_freq($channel),
        //    )
        //};

        let capture_rx = $capture_rx;

        let mut transfer_fin = $transfer_fin;
        let mut f_capturer = $f_capturer;

        //let (mut measure_time, mut f_ref, mut f_pwm) = $data_storage.lock(get_measure_settings);

        let f_ref = $f_ref;
        let mesure_period = $base_period * $base_period_devider - 1.millis();
        let measure_time =
            (mesure_period - $base_period / 4).min(config::MEASURE_TIME_MAX_MS.millis());

        let mut target = config::INITIAL_FREQMETER_TARGET;

        loop {
            defmt::trace!("{}: Waiting for start event...", $channel);
            {
                let mut counter = $base_period_devider - 1;
                while counter > 0 {
                    $start_event.wait().await;
                    counter -= 1;
                    if counter > 0 {
                        Mono::delay(1.millis()).await;
                    }
                }
            }
            let start_time = Mono::now();
            defmt::trace!("{}: Starting measurement cycle", $channel);

            // Start channel
            (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                transfer.accept_isr();
                transfer.start();
                capturer.start(target);
            });

            // process start event with timeout
            let start_capture = match Mono::timeout_after(
                (config::BASE_INTERVAL_MIN_MS / 2).millis(),
                capture_rx.recv(),
            )
            .await
            {
                Ok(Ok(c)) => c,
                Ok(Err(_)) => {
                    defmt::panic!("{}: Capture channel closed", $channel);
                }
                Err(_e) => {
                    defmt::warn!("{}: Capture timeout, channel down, reset...", $channel);
                    // Stop channel
                    (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                        capturer.stop();
                        transfer.stop();
                    });
                    target = config::INITIAL_FREQMETER_TARGET;
                    continue;
                }
            };

            // Wait result with timeout
            match Mono::timeout_at(start_time + mesure_period, capture_rx.recv()).await {
                Ok(Ok(c)) => {
                    defmt::trace!("{}: Got second capture: {}", $channel, c);
                    if let Ok((f, result)) = $freqmeter.calc_result(start_capture, c, f_ref) {
                        defmt::debug!("{}: Freq: {} Hz, result: {}", $channel, f, result);
                        target = $freqmeter.calc_new_target(
                            f,
                            measure_time,
                            config::INITIAL_FREQMETER_TARGET,
                        );
                        defmt::trace!("{}: New target: {}", $channel, target);
                        (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                            transfer.stop();
                            capturer.stop();
                        });
                    } else {
                        defmt::error!("{}: freqmeter overrun 2, reset...", $channel);
                        target = config::INITIAL_FREQMETER_TARGET;
                    }
                }
                Ok(Err(_)) => {
                    defmt::panic!("{}: Capture channel closed", $channel);
                }
                Err(_e) => {
                    defmt::warn!("{}: Capture timeout", $channel);
                    target = config::INITIAL_FREQMETER_TARGET;
                }
            }

            // Stop channel
            (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                capturer.stop();
                transfer.stop();
            });
            while let Ok(_) = capture_rx.try_recv() {} // flush channel to remove stale captures

            // update measure time
            //(measure_time, f_ref, f_pwm) = $data_storage.lock(get_measure_settings);
        }
    };
}
