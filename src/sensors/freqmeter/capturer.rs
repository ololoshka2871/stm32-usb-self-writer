use super::{
    capture::Capture,
    dma_traits::{PeriAddress, SafePeripheralRead},
    input_counter::InputCounter,
    tim_input_config_helper::{TimerControl, TimerInputConfig},
};

pub struct Capturer<TIM, const IN_TYPE: u8> {
    input: InputCounter<TIM, IN_TYPE>,
    master_cnt_reg_addr: u32,
    current_target: u16,
    master_extender: *const u16,
}

impl<TIM: TimerInputConfig + TimerControl, const IN_TYPE: u8> Capturer<TIM, IN_TYPE> {
    pub fn new(
        input: InputCounter<TIM, IN_TYPE>,
        master_cnt_reg_addr: u32,
        master_extender: *const u16,
    ) -> Self {
        Self {
            input,
            master_cnt_reg_addr,
            current_target: 1,
            master_extender,
        }
    }

    pub fn start(&mut self, new_target: u16) {
        self.stop();
        self.current_target = new_target;
        self.input.load_target(new_target - 1);
        self.input.load(new_target - 2);
        self.input.enable();
    }

    pub fn restart(&mut self) {
        self.stop();
        self.input.load_target(self.current_target - 1);
        self.input.load(self.current_target - 2);
        self.input.enable();
    }

    pub fn stop(&mut self) {
        self.input.disable();
    }

    pub fn capture_master(&self, dma_value: u16) -> Capture {
        let (extender_value, counter_value) = cortex_m::interrupt::free(|_| unsafe {
            (
                self.master_extender.read_volatile() as u32,
                (self.master_cnt_reg_addr as *const u16).read_volatile(),
            )
        });

        // Если захвачено достаточно больше чет текущее, то это значит что произошло переполнение пока суть до дело..
        // вычтем 1 из расширителя чтобы учесть это переполнение
        let extender_value = if counter_value < dma_value {
            extender_value.wrapping_sub(1)
        } else {
            extender_value
        };

        Capture {
            target: self.current_target,
            dma_value: (extender_value << 16) | dma_value as u32,
        }
    }
}

unsafe impl<TIM: TimerInputConfig + TimerControl, const IN_TYPE: u8> Send
    for Capturer<TIM, IN_TYPE>
{
}

// через этот трейт DMA будет знать адрес откуда копировать данные
unsafe impl<TIM: TimerInputConfig, const IN_TYPE: u8> PeriAddress for Capturer<TIM, IN_TYPE> {
    type MemSize = u32;

    #[inline(always)]
    fn address(&self) -> u32 {
        self.master_cnt_reg_addr
    }
}

// это маркер того что безопасно читать данные из этого периферийного устройства в любом контексте
impl<TIM: TimerInputConfig, const IN_TYPE: u8> SafePeripheralRead for Capturer<TIM, IN_TYPE> {}
