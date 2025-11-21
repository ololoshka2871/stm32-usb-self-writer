use crate::threads::sensor_processor::FChannel;
use corelogic::output_provider::{OutputProvider, OutputProviderError};

pub fn fill_output<E>(
    output: &mut super::messages::OutputResponse,
    get_output_values: &super::messages::OutputReq,
    output_provider: &impl OutputProvider<
        Output = crate::workmodes::output_storage::OutputStorage,
        Error = E,
    >,
) -> Result<(), OutputProviderError<E>> {
    let mut err: Option<OutputProviderError<E>> = None;

    if get_output_values.get_main_values.is_some() {
        match output_provider.with_output(|guard| {
            output.pressure =
                Some(guard.values[FChannel::Pressure as usize].unwrap_or(f64::NAN) as f32);
            output.temperature =
                Some(guard.values[FChannel::Temperature1 as usize].unwrap_or(f64::NAN) as f32);
            output.tcpu = Some(guard.t_cpu);
            output.vbat = Some(guard.vbat as f32);
        }) {
            Ok(()) => {}
            Err(e) => {
                output.pressure = Some(f32::NAN);
                output.temperature = Some(f32::NAN);
                output.tcpu = Some(f32::NAN);
                output.vbat = Some(f32::NAN);
                err = Some(OutputProviderError::Other(e));
            }
        }
    }

    if get_output_values.get_f.is_some() {
        match output_provider.with_output(|guard| {
            output.fp =
                Some(guard.frequencys[FChannel::Pressure as usize].unwrap_or_default() as f32);
            output.ft =
                Some(guard.frequencys[FChannel::Temperature1 as usize].unwrap_or_default() as f32);
        }) {
            Ok(()) => {}
            Err(e) => {
                output.fp = Some(f32::NAN);
                output.ft = Some(f32::NAN);
                err = Some(OutputProviderError::Other(e));
            }
        }
    }

    if get_output_values.get_raw.is_some() {
        match output_provider.with_output(|guard| {
            output.p_result = Some(super::messages::FreqmeterResult {
                target: guard.targets[FChannel::Pressure as usize],
                result: guard.results[FChannel::Pressure as usize].unwrap_or_default(),
            });
            output.t_result = Some(super::messages::FreqmeterResult {
                target: guard.targets[FChannel::Temperature1 as usize],
                result: guard.results[FChannel::Temperature1 as usize].unwrap_or_default(),
            });

            output.adc_tcpu = Some(guard.t_cpu_adc as u32);
            output.adc_vbat = Some(guard.vbat_adc as u32);
        }) {
            Ok(()) => {}
            Err(e) => {
                output.p_result = Some(super::messages::FreqmeterResult::default());
                output.t_result = Some(super::messages::FreqmeterResult::default());

                output.adc_tcpu = Some(u32::default());
                output.adc_vbat = Some(u32::default());
                err = Some(OutputProviderError::Other(e));
            }
        }
    }

    if let Some(err) = err {
        Err(err)
    } else {
        Ok(())
    }
}
