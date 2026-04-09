mod capture;
mod capturer;
mod input_counter;
mod states;

pub mod dma_traits;
pub mod freqmeter;
pub mod freqmeter_dma_l4;
pub mod master_counter;
pub mod tim_input_config_helper;

pub use capture::Capture;
pub use capturer::Capturer;
pub use freqmeter::{calc_new_target, calc_result};
pub use freqmeter_dma_l4::FreqmeterDmaChannelExt;
pub use input_counter::{ExtInputType, InputCounter, TimerInpitCounterExt};
pub use master_counter::*;
pub use states::FreqmeterStates;

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
macro_rules! build_freqmeter_dma {
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
            dma_traits::PeriAddress, Capture,
        };

        let mut capturer = $master_timer.make_capturer($input_timer);

        $dp.DBGMCU.$stop_reg.modify(|_, w| w.$stop_bit().set_bit());

        let buffer = unsafe { cortex_m::singleton!(: $master_type = 0).unwrap_unchecked() };

        let mut dma_transfer = $dma_channel;
        dma_transfer.configure_tim_up(buffer as *const _ as u32, capturer.address());

        (
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
        power_pin=$power_pin:expr,
        //data_storage=$data_storage:expr,
        transfer_fin=$transfer_fin:expr,
        f_capturer=$f_capturer:expr,
        f_ref=$f_ref:expr,
        mono=$mono:ty,
    ) => {
        use stm32_usb_self_writer::sensors::freqmeter::{
            calc_new_target, calc_result, FreqmeterDmaChannelExt, FreqmeterStates,
        };

        //let get_measure_settings = move |data_storage: &mut &mut data_storage::DataStorage| {
        //    (
        //        data_storage.holdings.get_measure_time($channel) as u64,
        //        data_storage.holdings.get_f_ref().hz(),
        //        data_storage.holdings.get_pwm_freq($channel),
        //    )
        //};

        let start_delay = 2.secs();
        let f_ref = $f_ref;

        let power_pin: &mut stm32_usb_self_writer::PowerCtrl = $power_pin;

        let mut transfer_fin = $transfer_fin;
        let mut f_capturer = $f_capturer;
        let mut capture_rx = $capture_rx;

        //let mut start_channel = |target| {
        //    (transfer_fin.get(), f_capturer.get()).lock(|transfer, capturer| {
        //        transfer.accept_isr();
        //        transfer.start();
        //        capturer.start(target);
        //    });
        //};
        //let mut stop_channel = || {
        //    (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
        //        capturer.stop();
        //        transfer.stop();
        //    });
        //};

        //let (mut measure_time, mut f_ref, mut f_pwm) = $data_storage.lock(get_measure_settings);

        let mut current_state = FreqmeterStates::<$mono>::init(start_delay);
        loop {
            match current_state {
                FreqmeterStates::PowerOff { remaining } => {
                    let now = <$mono>::now();
                    defmt::trace!(
                        "{}: Freqmeter: PowerOff, remaining: {} ms",
                        $channel,
                        remaining.to_millis()
                    );
                    power_pin.power_ctrl(false);
                    <$mono>::delay(remaining - FreqmeterStates::<$mono>::MIN_COLD_STARTUP_TIME)
                        .await;
                    let elapsed = <$mono>::now() - now;
                    current_state = FreqmeterStates::<$mono>::Preheating {
                        remaining: remaining - elapsed,
                    };
                }
                FreqmeterStates::Preheating { remaining } => {
                    let now = <$mono>::now();
                    defmt::trace!(
                        "{}: Freqmeter: Preheating, remaining: {} ms",
                        $channel,
                        remaining.to_millis()
                    );
                    power_pin.power_ctrl(true);
                    <$mono>::delay(remaining - FreqmeterStates::<$mono>::MIN_PREHEAT_TIME).await;
                    let elapsed = <$mono>::now() - now;
                    current_state = FreqmeterStates::<$mono>::adaptation(remaining, elapsed);
                }
                FreqmeterStates::Adaptation { remaining } => {
                    let now = <$mono>::now();
                    defmt::trace!(
                        "{}: Freqmeter: Adaptation, remaining: {} ms",
                        $channel,
                        remaining.to_millis()
                    );
                    (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                        transfer.accept_isr();
                        transfer.start();
                        capturer.start(config::INITIAL_FREQMETER_TARGET);
                    });

                    // process start event with timeout
                    let start_capture = match <$mono>::timeout_after(
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
                                while let Ok(_) = capture_rx.try_recv() {}
                            });
                            let elapsed = <$mono>::now() - now;
                            current_state =
                                FreqmeterStates::<$mono>::adaptation(remaining, elapsed);
                            continue;
                        }
                    };

                    // Wait result with timeout
                    let res = <$mono>::timeout_after(
                        (config::BASE_INTERVAL_MIN_MS / 2).millis(),
                        capture_rx.recv(),
                    )
                    .await;

                    let elapsed = <$mono>::now() - now;

                    match res {
                        Ok(Ok(capture)) => {
                            defmt::trace!("{}: Got second capture: {}", $channel, capture);
                            if let Ok((f, result)) = calc_result(start_capture, capture, f_ref) {
                                defmt::debug!(
                                    "{}: Adaptation done, elapsed: {} ms, Freq: {} Hz",
                                    $channel,
                                    elapsed.to_millis(),
                                    f
                                );
                                (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                                    transfer.stop();
                                    capturer.stop();
                                    while let Ok(_) = capture_rx.try_recv() {}
                                });
                                current_state = FreqmeterStates::Measure {
                                    prev_freq: f,
                                    remaining: remaining - elapsed,
                                };
                            } else {
                                defmt::error!("{}: freqmeter overrun 2, reset...", $channel);
                                current_state =
                                    FreqmeterStates::<$mono>::adaptation(remaining, elapsed);
                            }
                        }
                        Ok(Err(_)) => {
                            defmt::panic!("{}: Capture channel closed", $channel);
                        }
                        Err(_e) => {
                            defmt::warn!("{}: Capture timeout", $channel);
                            current_state =
                                FreqmeterStates::<$mono>::adaptation(remaining, elapsed);
                        }
                    }
                }
                FreqmeterStates::Measure {
                    prev_freq,
                    remaining,
                } => {
                    let remaining = if remaining < FreqmeterStates::<$mono>::MIN_MEASURE_TIME {
                        remaining.max(FreqmeterStates::<$mono>::MIN_MEASURE_TIME / 2)
                    } else {
                        let now = <$mono>::now();
                        $start_event.wait().await; // sync with rtc event
                        let elapsed = <$mono>::now() - now;

                        (remaining - elapsed)
                    };

                    let target = calc_new_target(
                        prev_freq,
                        remaining - FreqmeterStates::<$mono>::MIN_MEASURE_TIME / 2,
                        config::INITIAL_FREQMETER_TARGET,
                    );

                    defmt::trace!(
                        "{}: Freqmeter: Measure, prev_freq: {}, measure_time: {} ms, target: {}",
                        $channel,
                        prev_freq,
                        remaining.to_millis(),
                        target
                    );

                    let now = <$mono>::now();
                    (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                        transfer.accept_isr();
                        transfer.start();
                        capturer.start(target);
                    });

                    // process start event with timeout
                    let start_capture = match <$mono>::timeout_after(
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
                                while let Ok(_) = capture_rx.try_recv() {}
                            });
                            let elapsed = <$mono>::now() - now;
                            current_state =
                                FreqmeterStates::<$mono>::adaptation(remaining, elapsed);
                            continue;
                        }
                    };

                    // Wait result with timeout
                    let res = <$mono>::timeout_after(remaining, capture_rx.recv()).await;

                    let elapsed = <$mono>::now() - now;

                    match res {
                        Ok(Ok(capture)) => {
                            defmt::trace!("{}: Got second capture: {}", $channel, capture);
                            if let Ok((f, result)) = calc_result(start_capture, capture, f_ref) {
                                defmt::debug!(
                                    "{}: Measurment done: {} ms, Freq: {} Hz",
                                    $channel,
                                    elapsed.to_millis(),
                                    f
                                );
                                (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                                    transfer.stop();
                                    capturer.stop();
                                    while let Ok(_) = capture_rx.try_recv() {}
                                });

                                current_state = FreqmeterStates::<$mono>::plan_next_state(
                                    $base_period,
                                    $base_period_devider,
                                    f,
                                );
                            } else {
                                defmt::error!("{}: freqmeter overrun 2, reset...", $channel);
                                current_state =
                                    FreqmeterStates::<$mono>::adaptation(remaining, elapsed);
                            }
                        }
                        Ok(Err(_)) => {
                            defmt::panic!("{}: Capture channel closed", $channel);
                        }
                        Err(_e) => {
                            defmt::warn!("{}: Capture timeout", $channel);
                            current_state =
                                FreqmeterStates::<$mono>::adaptation(remaining, elapsed);
                        }
                    }
                }
            }
        }
    };
}
