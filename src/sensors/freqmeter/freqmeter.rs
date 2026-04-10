use stm32l4xx_hal::time::Hertz;

use super::capture::Capture;
use crate::config;

pub fn calc_result(start: Capture, capture: Capture, f_ref: Hertz) -> Result<(f32, u32), ()> {
    if start.target != capture.target {
        Err(())
    } else {
        let result = capture.wrapping_sub(start);
        let f = (capture.target as f32 * f_ref.0 as f32) / (result as f32);
        Ok((f, result))
    }
}

pub fn calc_new_target(f: f32, target_measure_time: config::Duration, min_target: u16) -> u16 {
    let target = (target_measure_time.to_nanos() as f32 / 1e9 * f) as u16;
    if target < min_target {
        min_target
    } else {
        target
    }
}
