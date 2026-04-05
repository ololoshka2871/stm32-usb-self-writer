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

impl<TIM, const IN_TYPE: u8> InputCounter<TIM, IN_TYPE> {
    pub fn from_timer(tim: TIM) -> Self {
        Self { tim }
    }
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

//impl_input_counters!(
//    TIM1: (PA8<Alternate<AF2>>, { ExtInputType::TI1FP1 as u8 }, tim1en, tim1rst, apb2enr, apb2rstr),
//    TIM1: (PA9<Alternate<AF2>>, { ExtInputType::TI2FP2 as u8 }, tim1en, tim1rst, apb2enr, apb2rstr),
//
//    TIM3: (PA6<Alternate<AF1>>, { ExtInputType::TI1FP1 as u8 }, tim3en, tim3rst, apb1enr, apb1rstr),
//    TIM3: (PA7<Alternate<AF1>>, { ExtInputType::TI2FP2 as u8 }, tim3en, tim3rst, apb1enr, apb1rstr),
//);
//
//impl<TIM: TimerInputConfig + TimerControl, const IN_TYPE: u8> InputCounter<TIM, IN_TYPE> {
//    #[allow(dead_code)]
//    pub fn load_counter(&mut self, value: u16) {
//        self.tim.write_count(value.into());
//    }
//
//    pub fn load_max(&mut self) {
//        self.tim.write_count(u16::MAX.into());
//    }
//
//    pub fn load_target(&mut self, value: u16) {
//        self.tim.set_auto_reload(value);
//    }
//
//    pub fn enable(&mut self) {
//        self.tim.reset_counter();
//        self.tim.enable_counter(true);
//    }
//
//    pub fn disable(&mut self) {
//        self.tim.enable_counter(false);
//    }
//
//    fn configure(&mut self) {
//        let ext_in_type = IN_TYPE.into();
//
//        self.tim.reset();
//
//        self.tim.set_edge(ext_in_type, Edge::Rising);
//        self.tim.enable_filter(ext_in_type);
//        self.tim.set_ext_input(ext_in_type);
//        self.tim.dma_request_ovf();
//
//        self.tim.set_auto_reload(5);
//    }
//}