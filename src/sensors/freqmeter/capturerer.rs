use super::{
    capture::Capture,
    dma_traits::{PeriAddress, SafePeripheralRead},
    input_counter::InputCounter,
    tim_input_config_helper::{TimerControl, TimerInputConfig},
};

pub struct Capturerer<TIM, const IN_TYPE: u8> {
    input: InputCounter<TIM, IN_TYPE>,
    master_cnt_reg_addr: u32,
    current_target: u16,
}

impl<TIM: TimerInputConfig + TimerControl, const IN_TYPE: u8> Capturerer<TIM, IN_TYPE> {
    pub fn new(input: InputCounter<TIM, IN_TYPE>, master_cnt_reg_addr: u32) -> Self {
        Self {
            input,
            master_cnt_reg_addr,
            current_target: 1,
        }
    }

    pub fn start(&mut self, new_target: u16) {
        self.current_target = new_target;
        self.input.load_max();
        self.input.load_target(new_target - 1);
        self.input.enable();
    }

    pub fn restart(&mut self) {
        self.input.load_max();
        self.input.load_target(self.current_target - 1);
        self.input.enable();
    }

    pub fn stop(&mut self) {
        self.input.disable();
    }

    pub fn capture(&self, dma_value: u32) -> Capture {
        let current_full_value = unsafe { (self.master_cnt_reg_addr as *const u32).read_volatile() };

        let current_full_value = if (current_full_value & 0x0000_FFFF) < dma_value {
            current_full_value.wrapping_sub(0x1_0000)
        } else {
            current_full_value
        };

        Capture {
            target: self.current_target,
            dma_value: (current_full_value & 0xFFFF_0000) | dma_value,
        }
    }
}

// через этот трейт DMA будет знать адрес откуда копировать данные
unsafe impl<TIM: TimerInputConfig, const IN_TYPE: u8> PeriAddress for Capturerer<TIM, IN_TYPE> {
    type MemSize = u32;

    fn address(&self) -> u32 {
        self.master_cnt_reg_addr
    }
}

// это маркер того что безопасно читать данные из этого периферийного устройства в любом контексте
impl<TIM: TimerInputConfig, const IN_TYPE: u8> SafePeripheralRead for Capturerer<TIM, IN_TYPE> {}
