use embedded_hal::digital::v2::OutputPin;
use rtic_monotonics::fugit::TimerDurationU64;
use stm32l4xx_hal::time::Hertz;

use crate::config;

use super::capture::Capture;

pub struct Freqmeter<PIN, const FREQ_HZ: u32> {
    power_pin: Option<PIN>,
}

impl<PIN: OutputPin, const FREQ_HZ: u32> Freqmeter<PIN, FREQ_HZ> {
    pub fn new() -> Self {
        Self { power_pin: None }
    }

    pub fn with_power_pin(pin: PIN) -> Self {
        Self {
            power_pin: Some(pin),
        }
    }

    pub fn power_ctrl(&mut self, on: bool) {
        if let Some(pin) = &mut self.power_pin {
            if on {
                pin.set_state(config::GENERATOR_ENABLE_LVL).ok();
            } else {
                pin.set_state(!config::GENERATOR_ENABLE_LVL).ok();
            }
        }
    }

    pub fn calc_result(
        &self,
        start: Capture,
        capture: Capture,
        f_ref: Hertz,
    ) -> Result<(f32, u32), ()> {
        if start.target != capture.target {
            Err(())
        } else {
            let result = capture.wrapping_sub(start);
            let f = (capture.target as f32 * f_ref.0 as f32) / (result as f32);
            Ok((f, result))
        }
    }

    pub fn calc_new_target(
        &self,
        f: f32,
        target_measure_time: TimerDurationU64<FREQ_HZ>,
        min_target: u16,
    ) -> u16 {
        let target = (target_measure_time.to_nanos() as f32 / 1e9 * f) as u16;
        if target < min_target {
            min_target
        } else {
            target
        }
    }
}
