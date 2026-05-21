#![allow(unused_imports)]
#![allow(unused_macros)]
#![allow(dead_code)]

macro_rules! master_timer {
    ( $name:ident, $ral_path:ident, $ral_steal_tgt:stmt, $hal_timer:ty ) => {
        pub mod $name {
            use stm32ral::*;
            use stm32l4xx_hal::timer::Timer;
            use crate::sensors::freqmeter::{
                InputCounter,
                Capturer,
                tim_input_config_helper::{TimerInputConfig, TimerControl},
            };

            pub type Type = u16;

            pub struct MasterCounter16(*mut u16);

            impl MasterCounter16 {
                pub fn new(_timer: Timer<$hal_timer>) -> Self {
                    let tgt = unsafe { $ral_steal_tgt };

                    // disable timer
                    stm32ral::modify_reg!($ral_path, tgt, CR1, CEN: Disabled);
                    // set prescaler 0 and autoreload MAX
                    stm32ral::write_reg!($ral_path, tgt, PSC, 0);
                    stm32ral::write_reg!($ral_path, tgt, ARR, u32::MAX);
                    // enable timer
                    stm32ral::modify_reg!($ral_path, tgt, CR1, CEN: Enabled);

                    let extender = unsafe { ::cortex_m::singleton!(: u16 = 0).unwrap_unchecked() };

                    Self(extender as *mut _)
                }

                pub fn make_capturer<TIM: TimerInputConfig + TimerControl, PIN, const IN_TYPE: u8>(
                    &self, input_counter: InputCounter<TIM, PIN, IN_TYPE>
                ) -> Capturer<TIM, PIN, IN_TYPE> {
                    let tgt = unsafe { $ral_steal_tgt };
                    Capturer::new(input_counter, &tgt.CNT as *const _ as u32, self.0 as *const _)
                }

                pub fn listen(&mut self) {
                    let tgt = unsafe { $ral_steal_tgt };
                    stm32ral::modify_reg!($ral_path, tgt, DIER, UIE: Enabled);
                }

                pub unsafe fn overflow_isr(&mut self) {
                    // increment extender
                    unsafe {self.0.write_volatile(self.0.read_volatile().wrapping_add(1)) };

                    // clear update flag
                    let tgt = unsafe { $ral_steal_tgt };
                    stm32ral::modify_reg!($ral_path, tgt, SR, UIF: Clear);
                }
            }
        }
    };
}

master_timer!(m_tim6, tim6, tim6::TIM6::steal(), stm32l4xx_hal::pac::TIM6);
master_timer!(m_tim7, tim7, tim7::TIM7::steal(), stm32l4xx_hal::pac::TIM7);
