use stm32l4xx_hal::time::Hertz;

use super::capture::Capture;
use crate::config;

pub fn calc_result(start: Capture, capture: Capture, f_ref: Hertz) -> Result<(f32, u32), ()> {
    if start.target != capture.target {
        Err(())
    } else {
        let result = capture.wrapping_sub(start);
        let f = (capture.target as f32 * f_ref.to_Hz() as f32) / (result as f32);
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

/// Try to correct result if it seems to have ±1 overflow error
/// Returns (frequency, corrected_result, was_corrected)
pub fn try_correct_overflow(
    result: u32,
    target: u16,
    f_ref: Hertz,
    prev_freq: f32,
) -> (f32, u32, bool) {
    let overflow_const = 65536u32;
    
    // Calculate current frequency
    let f_current = (target as f32 * f_ref.to_Hz() as f32) / (result as f32);
    let error_current = (f_current - prev_freq).abs();
    
    // Try result + overflow_const
    let result_plus = result.saturating_add(overflow_const);
    let f_plus = if result_plus > 0 {
        (target as f32 * f_ref.to_Hz() as f32) / (result_plus as f32)
    } else {
        0.0
    };
    let error_plus = (f_plus - prev_freq).abs();
    
    // Try result - overflow_const
    let result_minus = result.saturating_sub(overflow_const);
    let f_minus = if result_minus > 0 {
        (target as f32 * f_ref.to_Hz() as f32) / (result_minus as f32)
    } else {
        f32::MAX
    };
    let error_minus = (f_minus - prev_freq).abs();
    
    // Return the variant with smallest error
    if error_plus < error_current && error_plus < error_minus {
        (f_plus, result_plus, true)
    } else if error_minus < error_current {
        (f_minus, result_minus, true)
    } else {
        (f_current, result, false)
    }
}
