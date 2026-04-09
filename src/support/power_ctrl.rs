use crate::config;

pub trait PowerCtrl {
    fn power_ctrl(&mut self, on: bool);
}

impl<T: embedded_hal::digital::v2::OutputPin> PowerCtrl for T {
    fn power_ctrl(&mut self, on: bool) {
        if on {
            self.set_state(config::GENERATOR_ENABLE_LVL).ok();
        } else {
            self.set_state(config::GENERATOR_DISABLE_LVL).ok();
        }
    }
}
