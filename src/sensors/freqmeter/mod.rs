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
    ) => {{
        use stm32_usb_self_writer::sensors::freqmeter::FreqmeterDmaChannelExt;

        let buffer = $buffer;
        let capture: Capture = $capturer.lock(move |capturer| capturer.capture_master(buffer));

        let _ = $capture_tx.try_send(capture);

        $transfer.lock(|transfer| transfer.accept_isr());

        defmt::trace!("{}: DMA transfer complete", $channel);
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
        output_storage=$output_storage:expr,
        start_delay=$start_delay:expr,
        rtc=$rtc:expr,
        rtc_sync=$rtc_sync:expr,
        base_period=$base_period:expr,
        base_period_devider=$base_period_devider:expr,
        capture_rx=$capture_rx:expr,
        power_pin=$power_pin:expr,
        transfer_fin=$transfer_fin:expr,
        f_capturer=$f_capturer:expr,
        f_ref=$f_ref:expr,
        mono=$mono:ty,
    ) => {
        use stm32_usb_self_writer::sensors::freqmeter::{
            FreqmeterDmaChannelExt, FreqmeterStates, calc_new_target, calc_result,
        };

        let start_delay = $start_delay;
        let f_ref = $f_ref;

        let power_pin: &mut dyn stm32_usb_self_writer::PowerCtrl = $power_pin;

        let mut transfer_fin = $transfer_fin;
        let mut f_capturer = $f_capturer;
        let mut capture_rx = $capture_rx;

        let mut rtc = $rtc;
        let rtc_sync = $rtc_sync;

        let mut current_state = FreqmeterStates::<$mono>::init(start_delay);
        loop {
            match current_state {
                FreqmeterStates::PowerOff { deadline } => {
                    defmt::trace!(
                        "{}: Freqmeter: PowerOff, remaning: {} ms",
                        $channel,
                        rtc_sync.until_deadline_millis(deadline)
                    );
                    power_pin.power_ctrl(false);
                    <$mono>::delay_until(
                        deadline - FreqmeterStates::<$mono>::MIN_COLD_STARTUP_TIME,
                    )
                    .await;
                    current_state = FreqmeterStates::<$mono>::Preheating { deadline };
                }
                FreqmeterStates::Preheating { deadline } => {
                    defmt::trace!(
                        "{}: Freqmeter: Preheating, remaining: {} ms",
                        $channel,
                        rtc_sync.until_deadline_millis(deadline)
                    );
                    power_pin.power_ctrl(true);
                    <$mono>::delay_until(deadline - FreqmeterStates::<$mono>::MIN_PREHEAT_TIME)
                        .await;
                    current_state = FreqmeterStates::<$mono>::Adaptation { deadline };
                }
                FreqmeterStates::Adaptation { deadline } => {
                    defmt::trace!(
                        "{}: Freqmeter: Adaptation, remaining: {} ms",
                        $channel,
                        rtc_sync.until_deadline_millis(deadline)
                    );
                    (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                        transfer.accept_isr();
                        transfer.start();
                        capturer.start(config::INITIAL_FREQMETER_TARGET);
                    });

                    // process start event with timeout
                    let start_capture = match rtc_sync.timeout_at(deadline, capture_rx.recv()).await
                    {
                        Ok(Ok(c)) => c, // success
                        Ok(Err(_)) => {
                            defmt::panic!("{}: Capture channel closed", $channel);
                        }
                        Err(_e) => {
                            defmt::warn!(
                                "{}: Adaptation: Capture timeout, channel down, reset...",
                                $channel
                            );

                            $output_storage.lock(|output_storage| {
                                output_storage.set_freqmeter_result(
                                    $channel as usize,
                                    config::INITIAL_FREQMETER_TARGET as u32,
                                    None,
                                    None,
                                    rtc.lock(|rtc| rtc.current_time()),
                                )
                            });

                            // Stop channel
                            (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                                capturer.stop();
                                transfer.stop();
                                while let Ok(_) = capture_rx.try_recv() {}
                            });
                            current_state = FreqmeterStates::<$mono>::plan_next_state(
                                deadline,
                                $base_period,
                                $base_period_devider,
                                None,
                            );
                            continue;
                        }
                    };

                    // Wait result with timeout
                    match rtc_sync.timeout_at(deadline, capture_rx.recv()).await {
                        Ok(Ok(capture)) => {
                            defmt::trace!("{}: Got second capture: {}", $channel, capture);
                            if let Ok((f, result)) = calc_result(start_capture, capture, f_ref) {
                                defmt::debug!(
                                    "{}: Adaptation done, Freq: {} Hz, remaining: {} ms",
                                    $channel,
                                    f,
                                    rtc_sync.until_deadline_millis(deadline)
                                );
                                (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                                    transfer.stop();
                                    capturer.stop();
                                    while let Ok(_) = capture_rx.try_recv() {}
                                });

                                // successful adaptation
                                current_state = FreqmeterStates::Measure {
                                    prev_freq: f,
                                    deadline,
                                };
                                continue;
                            } else {
                                defmt::error!(
                                    "{}: Adaptation: freqmeter overrun 2, reset...",
                                    $channel
                                );
                            }
                        }
                        Ok(Err(_)) => {
                            defmt::panic!("{}: Capture channel closed", $channel);
                        }
                        Err(_e) => {
                            defmt::warn!("{}: Adaptation: Capture timeout", $channel);
                        }
                    }

                    // Failed: goto next measurement cycle
                    current_state = FreqmeterStates::<$mono>::plan_next_state(
                        deadline,
                        $base_period,
                        $base_period_devider,
                        None,
                    );

                    $output_storage.lock(|output_storage| {
                        output_storage.set_freqmeter_result(
                            $channel as usize,
                            config::INITIAL_FREQMETER_TARGET as u32,
                            None,
                            None,
                            rtc.lock(|rtc| rtc.current_time()),
                        )
                    });
                }
                FreqmeterStates::Measure {
                    prev_freq,
                    deadline,
                } => {
                    defmt::trace!(
                        "{}: Freqmeter: Measure, dedline at T={=u32:ms}, remaining: {} ms",
                        $channel,
                        deadline.ticks() * (1_000 / config::SYST_TIMER_HZ),
                        rtc_sync.until_deadline_millis(deadline)
                    );

                    let target = calc_new_target(
                        prev_freq,
                        rtc_sync.make_measure_time(deadline),
                        config::INITIAL_FREQMETER_TARGET,
                    );
                    if target < 2 {
                        defmt::error!("{}: Target too low, reset...", $channel);
                        current_state = FreqmeterStates::<$mono>::plan_next_state(
                            deadline,
                            $base_period,
                            $base_period_devider,
                            None,
                        );

                        $output_storage.lock(|output_storage| {
                            output_storage.set_freqmeter_result(
                                $channel as usize,
                                target as u32,
                                None,
                                Some(prev_freq as f64),
                                rtc.lock(|rtc| rtc.current_time()),
                            )
                        });

                        continue;
                    }

                    defmt::trace!(
                        "{}: Freqmeter: Measure, prev_freq: {}, measure_time: {} ms, target: {}",
                        $channel,
                        prev_freq,
                        rtc_sync.until_deadline_millis(deadline),
                        target
                    );

                    (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                        transfer.accept_isr();
                        transfer.start();
                        capturer.start(target);
                    });

                    // process start event with timeout
                    let start_capture = match rtc_sync.timeout_at(deadline, capture_rx.recv()).await
                    {
                        Ok(Ok(c)) => c, // success
                        Ok(Err(_)) => {
                            defmt::panic!("{}: Measure: Capture channel closed", $channel);
                        }
                        Err(_e) => {
                            defmt::warn!("{}: Measure: Capture timeout, reset...", $channel);
                            // Stop channel
                            (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                                capturer.stop();
                                transfer.stop();
                                while let Ok(_) = capture_rx.try_recv() {}
                            });

                            $output_storage.lock(|output_storage| {
                                output_storage.set_freqmeter_result(
                                    $channel as usize,
                                    target as u32,
                                    None,
                                    Some(prev_freq as f64),
                                    rtc.lock(|rtc| rtc.current_time()),
                                )
                            });

                            current_state = FreqmeterStates::<$mono>::plan_next_state(
                                <$mono>::now(),
                                $base_period,
                                $base_period_devider,
                                None,
                            );
                            continue;
                        }
                    };

                    // Wait result with timeout
                    match rtc_sync.timeout_at(deadline, capture_rx.recv()).await {
                        Ok(Ok(capture)) => {
                            defmt::trace!("{}: Measure: Got second capture: {}", $channel, capture);
                            if let Ok((f, result)) = calc_result(start_capture, capture, f_ref) {
                                defmt::trace!("{}: Measurment done: Freq: {} Hz", $channel, f);
                                (&mut transfer_fin, &mut f_capturer).lock(|transfer, capturer| {
                                    transfer.stop();
                                    capturer.stop();
                                    while let Ok(_) = capture_rx.try_recv() {}
                                });

                                $output_storage.lock(|output_storage| {
                                    output_storage.set_freqmeter_result(
                                        $channel as usize,
                                        target as u32,
                                        Some(result),
                                        Some(f as f64),
                                        rtc.lock(|rtc| rtc.current_time()),
                                    )
                                });

                                current_state = FreqmeterStates::<$mono>::plan_next_state(
                                    deadline,
                                    $base_period,
                                    $base_period_devider,
                                    Some(f),
                                );

                                rtc_sync.delay_until_sync(deadline).await;
                                continue;
                            } else {
                                defmt::error!(
                                    "{}: Measure: Freqmeter overrun 2, reset...",
                                    $channel
                                );
                            }
                        }
                        Ok(Err(_)) => {
                            defmt::panic!("{}: Capture channel closed", $channel);
                        }
                        Err(_e) => {
                            defmt::warn!(
                                "{}: Measure: Capture timeout, deadline reached",
                                $channel
                            );
                        }
                    }
                    current_state = FreqmeterStates::<$mono>::plan_next_state(
                        <$mono>::now(),
                        $base_period,
                        $base_period_devider,
                        None,
                    );

                    // TODO: report F = Some(prev_freq)
                }
            }
        }
    };
}
