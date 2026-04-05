#![allow(unused_imports)]
#![allow(unused_macros)]
#![allow(dead_code)]

macro_rules! master_timer {
    ( $name:ident, $ral_path:ident, $ral_steal_tgt:stmt, $hal_timer:ty ) => {
        pub mod $name {
            use stm32ral::*;
            use stm32l4xx_hal::timer::Timer;
            use crate::drivers::{
                InputCounter,
                Capturer16,
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

                pub fn make_capturer<TIM: TimerInputConfig + TimerControl, const IN_TYPE: u8>(&self, input_counter: InputCounter<TIM, IN_TYPE>) -> Capturer16<TIM, IN_TYPE> {
                    let tgt = unsafe { $ral_steal_tgt };
                    Capturer16::new(input_counter, &tgt.CNT as *const _ as u32, self.0 as *const _)
                }

                pub fn listen(&mut self) {
                    let tgt = unsafe { $ral_steal_tgt };
                    stm32ral::modify_reg!($ral_path, tgt, DIER, UIE: Enabled);
                }

                pub unsafe fn overflow(&mut self) {
                    // increment extender
                    self.0.write_volatile(self.0.read_volatile().wrapping_add(1));

                    // clear update flag
                    let tgt = unsafe { $ral_steal_tgt };
                    stm32ral::modify_reg!($ral_path, tgt, SR, UIF: Clear);
                    stm32ral::modify_reg!($ral_path, tgt, SR, UIF: Clear); // Я не знаю почему, но настоящей STMке без этого не работает.
                }
            }
        }
    };
}

//master_timer!(m_tim1, tim1, tim1::TIM1::steal(), stm32f1xx_hal::pac::TIM1);
//master_timer!(m_tim2, tim2, tim2::TIM2::steal(), stm32f1xx_hal::pac::TIM2);
//master_timer!(m_tim3, tim3, tim3::TIM3::steal(), stm32f1xx_hal::pac::TIM3);
//master_timer!(m_tim4, tim4, tim4::TIM4::steal(), stm32f1xx_hal::pac::TIM4);