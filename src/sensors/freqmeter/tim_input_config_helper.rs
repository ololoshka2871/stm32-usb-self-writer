use crate::sensors::freqmeter::input_counter::ExtInputType;

#[derive(Clone, Copy, defmt::Format)]
pub enum Edge {
    Rising,
    Falling,
}

pub trait TimerInputConfig {
    fn reset(&mut self);
    fn set_edge(&mut self, ext_input: ExtInputType, edge: Edge);
    fn enable_filter(&mut self, ext_input: ExtInputType);
    fn set_ext_input(&mut self, ext_input: ExtInputType);
    fn dma_request_ovf(&mut self);
}

pub trait TimerControl {
    fn reset_counter(&mut self);
    fn write_count(&mut self, value: u16);
    fn enable_counter(&mut self, enable: bool);
    fn set_auto_reload(&mut self, value: u16);
}


//macro_rules! in_timer {
//    ($TIM:ty) => {
//        impl TimerInputConfig for $TIM {
//            fn reset(&mut self) {
//                let regs = unsafe { &*<$TIM>::ptr() };
//
//                regs.smcr.modify(|_, w| unsafe {
//                    w.sms()
//                        .disabled()
//                        .ts()
//                        .bits(0b000)
//                        .etf()
//                        .bits(0b000)
//                        .etps()
//                        .div1()
//                        .ece()
//                        .clear_bit()
//                        .etp()
//                        .clear_bit()
//                });
//
//                regs.cr1.modify(|_, w| {
//                    w.ckd()
//                        .div1()
//                        .cms()
//                        .edge_aligned()
//                        .dir()
//                        .up()
//                        .opm()
//                        .clear_bit()
//                        .urs()
//                        .set_bit() // update event generation disable
//                        .udis()
//                        .clear_bit()
//                });
//            }
//
//            fn set_edge(&mut self, ext_input: ExtInputType, edge: Edge) {
//                let regs = unsafe { &*<$TIM>::ptr() };
//
//                // output enable = false
//                match ext_input {
//                    ExtInputType::TI1FP1 => {
//                        regs.ccer.modify(|_, w| w.cc1e().clear_bit());
//                        match edge {
//                            Edge::Rising => regs.ccer.modify(|_, w| w.cc1p().clear_bit()),
//                            Edge::Falling => regs.ccer.modify(|_, w| w.cc1p().set_bit()),
//                            Edge::RisingFalling => regs
//                                .ccer
//                                .modify(|_, w| w.cc1p().set_bit().cc1np().set_bit()),
//                        }
//                    }
//                    ExtInputType::TI2FP2 => {
//                        regs.ccer.modify(|_, w| w.cc2e().clear_bit());
//                        match edge {
//                            Edge::Rising => regs.ccer.modify(|_, w| w.cc2p().clear_bit()),
//                            Edge::Falling => regs.ccer.modify(|_, w| w.cc2p().set_bit()),
//                            Edge::RisingFalling => regs
//                                .ccer
//                                .modify(|_, w| w.cc2p().set_bit().cc1np().set_bit()),
//                        }
//                    }
//                }
//            }
//
//            fn enable_filter(&mut self, ext_input: ExtInputType) {
//                let regs = unsafe { &*<$TIM>::ptr() };
//                match ext_input {
//                    ExtInputType::TI1FP1 => regs.ccmr1_input().modify(|_, w| w.ic1f().fck_int_n8()),
//                    ExtInputType::TI2FP2 => regs.ccmr1_input().modify(|_, w| w.ic2f().bits(0b0100)),
//                }
//            }
//
//            fn set_ext_input(&mut self, ext_input: ExtInputType) {
//                let regs = unsafe { &*<$TIM>::ptr() };
//
//                // Slave mode selection: External clock mode 1
//                regs.smcr.modify(|_, w| w.sms().ext_clock_mode());
//
//                match ext_input {
//                    ExtInputType::TI1FP1 => regs.smcr.modify(|_, w| w.ts().ti1fp1()),
//                    ExtInputType::TI2FP2 => regs.smcr.modify(|_, w| w.ts().ti2fp2()),
//                }
//            }
//
//            fn dma_request_ovf(&mut self) {
//                let regs = unsafe { &*<$TIM>::ptr() };
//                regs.sr.modify(|_, w| w.uif().clear_bit());
//                regs.dier.modify(|_, w| w.ude().set_bit());
//            }
//        }
//
//        impl TimerControl for $TIM {
//            fn reset_counter(&mut self) {
//                let regs = unsafe { &*<$TIM>::ptr() };
//                regs.cnt.write(|w| w.cnt().bits(0));
//            }
//
//            fn write_count(&mut self, value: u16) {
//                let regs = unsafe { &*<$TIM>::ptr() };
//                regs.cnt.write(|w| w.cnt().bits(value));
//            }
//
//            fn enable_counter(&mut self, enable: bool) {
//                let regs = unsafe { &*<$TIM>::ptr() };
//                regs.cr1.modify(|_, w| w.cen().bit(enable));
//            }
//
//            fn set_auto_reload(&mut self, value: u16) {
//                let regs = unsafe { &*<$TIM>::ptr() };
//                regs.arr.write(|w| w.arr().bits(value));
//            }
//        }
//    };
//}
//
//in_timer! { pac::TIM1 }
//in_timer! { pac::TIM3 }