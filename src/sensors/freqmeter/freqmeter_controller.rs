use core::fmt::Debug;
use core::marker::PhantomData;

use freertos_rust::{Duration, Timer};
use stm32l4xx_hal::{gpio::PinState, prelude::OutputPin};

use crate::{support::timer_period::TimerExt, threads::sensor_processor::FChannel};

use super::{f_ch_processor::FChProcessor, hw_in_counters::InCounter, TimerEvent};

pub struct FreqmeterController<'a, TIM, DMA, INPIN, ENPIN>
where
    TIM: InCounter<DMA, INPIN>,
{
    freqmeter: &'a mut TIM,
    gpio_pin: ENPIN,
    prev: u32,
    prev_diff: Option<u32>,
    start: bool,
    no_signal_guard: Timer,
    current_guard_ticks: u32,
    _phantomdata1: PhantomData<DMA>,
    _phantomdata2: PhantomData<INPIN>,
}

impl<'a, TIM, DMA, INPIN, ENPIN> FChProcessor for FreqmeterController<'a, TIM, DMA, INPIN, ENPIN>
where
    TIM: InCounter<DMA, INPIN>,
    ENPIN: OutputPin,
    <ENPIN as OutputPin>::Error: Debug,
{
    fn power_on(&mut self) {
        self.set_lvl(crate::config::GENERATOR_ENABLE_LVL);
    }

    fn start(&mut self) {
        self.start = true;
        self.freqmeter.cold_start();
        let _ = self.no_signal_guard.start(Duration::infinite());
    }

    fn diasble(&mut self) {
        let _ = self.no_signal_guard.stop(Duration::infinite());
        self.freqmeter.stop();
        self.set_lvl(crate::config::GENERATOR_DISABLE_LVL);
    }

    fn enabled(&self) -> bool {
        self.start
    }

    fn restart(&mut self) {
        self.start = true;
        self.reset_guard();
        self.freqmeter.cold_start();
    }

    fn reset_guard(&mut self) {
        let _ = self.no_signal_guard.stop(Duration::infinite());
        let _ = self.no_signal_guard.start(Duration::infinite());
    }

    fn set_target(&mut self, new_target: u32, guard_ticks: u32) {
        let _ = self.no_signal_guard.stop(Duration::infinite());
        self.freqmeter.stop();
        self.current_guard_ticks = guard_ticks;

        let _ = self
            .no_signal_guard
            .change_period(Duration::infinite(), Duration::ticks(guard_ticks));

        self.freqmeter.set_target32(new_target);
        self.restart();
    }

    #[allow(unused_mut)]
    #[allow(unused_assignments)]
    fn input_captured(&mut self, mut event: TimerEvent, mut captured: u32) -> Option<u32> {
        #[cfg(not(feature = "freqmeter-start-stop"))]
        {
            event = if self.start {
                self.start = false;
                TimerEvent::Start
            } else {
                TimerEvent::Stop
            };
        }

        match event {
            TimerEvent::Start => {
                self.prev = captured;
                None
            }
            TimerEvent::Stop => {
                let mut diff = if self.prev <= captured {
                    captured - self.prev
                } else {
                    u32::MAX - self.prev + captured
                };

                {
                    const DETECT_WINDOW: u32 = 500;
                    const MASTER_SELF_MAX_VALUE: u32 = u16::MAX as u32 + 1;

                    if let Some(pd) = &self.prev_diff {
                        let diff2prev_diff = pd.abs_diff(diff);
                        if diff2prev_diff <= MASTER_SELF_MAX_VALUE + DETECT_WINDOW
                            && diff2prev_diff >= MASTER_SELF_MAX_VALUE - DETECT_WINDOW
                        {
                            // Или может забраковать измерение?

                            defmt::warn!(
                                "0xffff + {} wrap diff detected ({} -> {}), fixing!",
                                diff2prev_diff as i32 - MASTER_SELF_MAX_VALUE as i32,
                                pd,
                                diff
                            );
                            if *pd > diff {
                                // Иголка вниз
                                captured += MASTER_SELF_MAX_VALUE;
                                diff += MASTER_SELF_MAX_VALUE;
                            } else {
                                // Иголка вверх
                                captured -= MASTER_SELF_MAX_VALUE;
                                diff -= MASTER_SELF_MAX_VALUE;
                            }
                            self.prev_diff = None;
                        } else {
                            self.prev_diff = Some(diff);
                        }
                    } else {
                        self.prev_diff = Some(diff);
                    }
                }

                self.prev = captured;

                Some(diff)
            }
        }
    }

    fn target(&self) -> u32 {
        self.freqmeter.target32()
    }
}

impl<'a, TIM, DMA, INPIN, ENPIN> FreqmeterController<'a, TIM, DMA, INPIN, ENPIN>
where
    TIM: InCounter<DMA, INPIN>,
    ENPIN: OutputPin,
    <ENPIN as OutputPin>::Error: Debug,
{
    pub fn new<F>(
        freqmeter: &'a mut TIM,
        gpio_pin: ENPIN,
        ch: FChannel,
        initial_guard_ticks: u32,
        f: F,
    ) -> Self
    where
        F: Fn(u32) + Send + 'static,
    {
        let timer = crate::support::new_freertos_timer(
            Duration::ticks(initial_guard_ticks),
            ch.into(),
            move |timer| f(timer.period()),
        );
        let _ = timer.stop(Duration::infinite());

        Self {
            freqmeter,
            gpio_pin,
            prev: 0,
            prev_diff: None,
            start: false,
            no_signal_guard: timer,
            current_guard_ticks: initial_guard_ticks,
            _phantomdata1: PhantomData,
            _phantomdata2: PhantomData,
        }
    }

    fn set_lvl(&mut self, lvl: PinState) {
        let _ = match lvl {
            PinState::High => self.gpio_pin.set_high(),
            PinState::Low => self.gpio_pin.set_low(),
        };
    }
}
