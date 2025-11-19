use crate::config::INPUT_CHANNEL_COUNT;

pub struct OutputStorage {
    pub targets: [u32; INPUT_CHANNEL_COUNT],
    pub results: [Option<u32>; INPUT_CHANNEL_COUNT],
    pub frequencys: [Option<f64>; INPUT_CHANNEL_COUNT],
    pub values: [Option<f64>; INPUT_CHANNEL_COUNT],

    pub t_cpu: f32,
    pub t_cpu_adc: u16,

    pub vbat: f32,
    pub vbat_adc: u16,
}

impl Default for OutputStorage {
    fn default() -> Self {
        Self {
            targets: [crate::config::INITIAL_FREQMETER_TARGET; INPUT_CHANNEL_COUNT],
            results: [None; INPUT_CHANNEL_COUNT],
            frequencys: [None; INPUT_CHANNEL_COUNT],
            values: [None; INPUT_CHANNEL_COUNT],

            t_cpu: 0.0,
            t_cpu_adc: 0,
            vbat: 0.0,
            vbat_adc: 0,
        }
    }
}
