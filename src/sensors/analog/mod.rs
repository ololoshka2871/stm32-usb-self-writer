use cortex_m::{delay, prelude::*};
use embedded_hal::blocking::delay::DelayUs;
use stm32l4xx_hal::adc::{Channel, Resolution, SampleTime, Temperature, Vref, ADC};

pub struct AnalogSensor<BATTERY_PIN> {
    adc: ADC,
    tcpu_ch: Temperature,
    v_ref: Vref,
    vbat_pin: BATTERY_PIN,
}

impl<BATTERY_PIN: Send + Channel> AnalogSensor<BATTERY_PIN> {
    pub fn new(mut adc: ADC, vbat_pin: BATTERY_PIN, delay: &mut impl DelayUs<u32>) -> Self {
        adc.set_sample_time(SampleTime::Cycles640_5);
        adc.set_resolution(Resolution::Bits12);

        let tcpu_ch = adc.enable_temperature(delay);
        let v_ref = adc.enable_vref(delay);

        Self {
            adc,
            tcpu_ch,
            v_ref,
            vbat_pin,
        }
    }

    pub fn read(&mut self) -> (f32, f32) {
        self.adc.calibrate(&mut self.v_ref);

        let v = self.adc.read(&mut self.vbat_pin).unwrap_or(0);
        let vbat_input_v = self.adc.to_millivolts(v) as f32 / 1000.0;
        let vbat = vbat_input_v * (crate::config::VBAT_DEVIDER_R1 + crate::config::VBAT_DEVIDER_R2)
            / crate::config::VBAT_DEVIDER_R2;
        let v = self.adc.read(&mut self.tcpu_ch).unwrap_or(0);
        let tcpu = self.adc.to_degrees_centigrade(v);

        (vbat, tcpu)
    }
}
