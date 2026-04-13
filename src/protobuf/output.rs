use crate::workmodes::{FChannel, output_storage::OutputStorage};

pub fn fill_output(
    output: &mut super::messages::OutputResponse,
    req: &super::messages::OutputReq,
    output_storage: &OutputStorage,
) {
    if req.get_main_values.is_some() {
        output.pressure =
            Some(output_storage.values[FChannel::Pressure as usize].unwrap_or(f64::NAN) as f32);
        output.temperature =
            Some(output_storage.values[FChannel::Temperature as usize].unwrap_or(f64::NAN) as f32);
        output.tcpu = Some(output_storage.t_cpu);
        output.vbat = Some(output_storage.vbat as f32);
    }

    if req.get_f.is_some() {
        output.fp =
            Some(output_storage.frequencys[FChannel::Pressure as usize].unwrap_or_default() as f32);
        output.ft = Some(
            output_storage.frequencys[FChannel::Temperature as usize].unwrap_or_default() as f32,
        );
        output.ftimestamp = Some(output_storage.freq_timestamp);
    }

    if req.get_raw.is_some() {
        output.p_result = Some(super::messages::FreqmeterResult {
            target: output_storage.targets[FChannel::Pressure as usize],
            result: output_storage.results[FChannel::Pressure as usize].unwrap_or_default(),
        });
        output.t_result = Some(super::messages::FreqmeterResult {
            target: output_storage.targets[FChannel::Temperature as usize],
            result: output_storage.results[FChannel::Temperature as usize].unwrap_or_default(),
        });

        output.adc_tcpu = Some(output_storage.t_cpu_adc as u32);
        output.adc_vbat = Some(output_storage.vbat_adc as u32);
    }
}
