use rtic_monotonics::fugit::TimerDurationU64;
use stm32l4xx_hal::time::Hertz;

use super::capture::Capture;

pub struct Freqmeter {
    start_value: Option<Capture>,
}

impl Freqmeter {
    pub fn new() -> Self {
        Self { start_value: None }
    }

    pub fn reset(&mut self) {
        self.start_value = None;
    }

    pub fn feed(&mut self, capture: Capture, f_ref: Hertz) -> Option<(f32, u32)> {
        match self.start_value {
            Some(start) => {
                if start.target != capture.target {
                    self.start_value.replace(capture);
                    None
                } else {
                    let result = capture.wrapping_sub(start);

                    self.start_value.replace(capture);
                    let f = (capture.target as f32 * f_ref.0 as f32) / (result as f32);
                    Some((f, result))
                }
            }
            None => {
                self.start_value.replace(capture);
                None
            }
        }
    }

    pub fn calc_new_target<const FREQ_HZ: u32>(
        &self,
        f: f32,
        target_measure_time: TimerDurationU64<FREQ_HZ>,
        min_target: u16,
        old_target: u16,
    ) -> Option<u16> {
        let target = (target_measure_time.to_nanos() as f32 / 1e9 * f) as u16;
        if target < min_target {
            Some(min_target)
        } else if target.abs_diff(old_target) < min_target {
            None
        } else {
            Some(target)
        }
    }
}
