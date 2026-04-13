use crate::clocking::rtc::CurrentTime;

const CHANNEL_COUNT: usize = 2;

#[derive(Clone, Debug)]
pub struct OutputStorage {
    pub targets: [u32; CHANNEL_COUNT],
    pub results: [Option<u32>; CHANNEL_COUNT],
    pub frequencys: [Option<f64>; CHANNEL_COUNT],
    pub freq_timestamps: [CurrentTime; CHANNEL_COUNT],
    pub values: [Option<f64>; CHANNEL_COUNT],

    pub t_cpu: f32,
    pub t_cpu_adc: u16,

    pub vbat: f32,
    pub vbat_adc: u16,
}

impl Default for OutputStorage {
    fn default() -> Self {
        Self {
            targets: [crate::config::INITIAL_FREQMETER_TARGET as u32; CHANNEL_COUNT],
            results: [None; CHANNEL_COUNT],
            frequencys: [None; CHANNEL_COUNT],
            freq_timestamps: [CurrentTime::default(); CHANNEL_COUNT],
            values: [None; CHANNEL_COUNT],

            t_cpu: 0.0,
            t_cpu_adc: 0,
            vbat: 0.0,
            vbat_adc: 0,
        }
    }
}

impl OutputStorage {
    pub fn set_analog_values(&mut self, vbat: f32, tcpu: f32, vbat_adc: u16, tcpu_adc: u16) {
        self.vbat = vbat;
        self.t_cpu = tcpu;
        self.vbat_adc = vbat_adc;
        self.t_cpu_adc = tcpu_adc;
    }

    pub fn set_freqmeter_result(
        &mut self,
        channel: usize,
        target: u32,
        result: Option<u32>,
        f: Option<f64>,
        timestamp: CurrentTime,
    ) {
        self.targets[channel] = target;
        self.results[channel] = result;
        self.frequencys[channel] = f;
        self.freq_timestamps[channel] = timestamp;
    }

    pub fn freq_timestamp(&self) -> u64 {
        // return max timestamp of channels
        self.freq_timestamps
            .iter()
            .map(|&ts| ts.into())
            .max()
            .unwrap_or_default()
    }
}
