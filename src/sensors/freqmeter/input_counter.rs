use stm32l4xx_hal::{
    gpio::{Alternate, PushPull, PA1, PA5, PA8, PA9},
    pac::{RCC, TIM1, TIM2},
};

use super::tim_input_config_helper::{Edge, TimerControl, TimerInputConfig};

#[allow(unused)]
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum ExtInputType {
    TI1FP1 = 0,
    TI2FP2 = 1,
}

impl From<u8> for ExtInputType {
    fn from(value: u8) -> Self {
        match value {
            0 => ExtInputType::TI1FP1,
            1 => ExtInputType::TI2FP2,
            _ => panic!("Invalid value"),
        }
    }
}

pub trait TimerInpitCounterExt<PIN, const IN_TYPE: u8>: Sized {
    fn into_input_counter(self, _pin: PIN) -> InputCounter<Self, IN_TYPE> {
        InputCounter { tim: self }
    }
}

pub struct InputCounter<TIM, const IN_TYPE: u8> {
    tim: TIM,
}

impl<TIM: TimerInputConfig + TimerControl, const IN_TYPE: u8> InputCounter<TIM, IN_TYPE> {
    pub fn configure(&mut self) {
        let ext_in_type = IN_TYPE.into();

        self.tim.reset();
        self.tim.set_edge(ext_in_type, Edge::Rising);
        self.tim.enable_filter(ext_in_type);
        self.tim.set_ext_input(ext_in_type);
        self.tim.dma_request_ovf();
    }

    pub fn load_target(&mut self, target: u16) {
        self.tim.set_auto_reload(target);
        self.tim.reset_counter();
    }

    pub fn load_max(&mut self) {
        self.tim.set_auto_reload(u16::MAX);
        self.tim.reset_counter();
    }

    pub fn enable(&mut self) {
        self.tim.enable_counter(true);
    }

    pub fn disable(&mut self) {
        self.tim.enable_counter(false);
    }

    pub fn into_inner(self) -> TIM {
        self.tim
    }
}

macro_rules! impl_input_counters {
    ($($TIM:ident: ($PIN:ty, $EXT_IN_TYPE:expr, $timXen:ident, $timXrst:ident, $apbenr:ident, $apbrstr:ident),)+) => {
        $(
            impl TimerInpitCounterExt<$PIN, $EXT_IN_TYPE> for $TIM {
                fn into_input_counter(
                    self,
                    _pin: $PIN,
                ) -> InputCounter<Self, $EXT_IN_TYPE> {
                    InputCounter::<Self, $EXT_IN_TYPE>::new(self)
                }
            }

            impl InputCounter<$TIM, $EXT_IN_TYPE> {
                pub fn new(tim: $TIM) -> Self {
                    let rcc = unsafe { &*RCC::ptr() };

                    rcc.$apbenr.modify(|_, w| w.$timXen().set_bit());
                    rcc.$apbrstr.modify(|_, w| w.$timXrst().set_bit());
                    rcc.$apbrstr.modify(|_, w| w.$timXrst().clear_bit());

                    let mut t = Self { tim };
                    t.configure();
                    t
                }
            }
        )+
    };
}

impl_input_counters!(
    TIM1: (PA8<Alternate<PushPull, 1>>, { ExtInputType::TI1FP1 as u8 }, tim1en, tim1rst, apb2enr, apb2rstr),
    TIM1: (PA9<Alternate<PushPull, 1>>, { ExtInputType::TI2FP2 as u8 }, tim1en, tim1rst, apb2enr, apb2rstr),

    // FIXME
    TIM2: (PA5<Alternate<PushPull, 1>>, { ExtInputType::TI1FP1 as u8 }, tim2en, tim2rst, apb1enr1, apb1rstr1),
    TIM2: (PA1<Alternate<PushPull, 1>>, { ExtInputType::TI2FP2 as u8 }, tim2en, tim2rst, apb1enr1, apb1rstr1),
);
