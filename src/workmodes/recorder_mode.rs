use alloc::sync::Arc;

#[allow(unused_imports)]
use freertos_rust::{Duration, Mutex, Task, TaskPriority};
use stm32l4xx_hal::{
    adc::ADC,
    prelude::*,
    rcc::{Enable, PllConfig, Reset},
    stm32,
    stm32l4::stm32l4x3::Peripherals,
    time::Hertz,
};

#[allow(unused_imports)]
use stm32l4xx_hal::gpio::{
    Alternate, Analog, Output, PushPull, Speed, PA0, PA1, PA2, PA3, PA6, PA7, PA8, PB0, PC10, PD10,
    PD11, PD13, PE12,
};

#[allow(unused_imports)]
use crate::{
    sensors::freqmeter::master_counter,
    support::{interrupt_controller::IInterruptController, InterruptController},
    threads::{self, free_rtos_delay::FreeRtosDelay},
    workmodes::processing::RecorderProcessor,
};

use super::{common::ClockConfigProvider, output_storage::OutputStorage, WorkMode};

#[allow(unused)]
pub struct RecorderMode {
    rcc: stm32l4xx_hal::rcc::Rcc,
    flash: Arc<Mutex<stm32l4xx_hal::flash::Parts>>,
    pwr: stm32l4xx_hal::pwr::Pwr,

    clocks: Option<stm32l4xx_hal::rcc::Clocks>,

    interrupt_controller: Arc<dyn IInterruptController>,

    crc: Arc<Mutex<stm32l4xx_hal::crc::Crc>>,

    in_p: PA8<Alternate<PushPull, 1>>,
    in_t: PA0<Alternate<PushPull, 1>>,
    en_p: PD13<Output<PushPull>>,
    en_t: PD10<Output<PushPull>>,
    dma1_ch2: stm32l4xx_hal::dma::dma1::C2,
    dma1_ch6: stm32l4xx_hal::dma::dma1::C6,
    timer1: stm32l4xx_hal::stm32l4::stm32l4x3::TIM1,
    timer2: stm32l4xx_hal::stm32l4::stm32l4x3::TIM2,

    adc: stm32l4xx_hal::stm32::ADC1,
    adc_common: stm32l4xx_hal::device::ADC_COMMON,
    vbat_pin: PA1<Analog>,

    #[cfg(not(feature = "no-flash"))]
    qspi: qspi_stm32lx3::qspi::Qspi<(
        PA3<Alternate<PushPull, 10>>,
        PA2<Alternate<PushPull, 10>>,
        PE12<Alternate<PushPull, 10>>,
        PB0<Alternate<PushPull, 10>>,
        PA7<Alternate<PushPull, 10>>,
        PA6<Alternate<PushPull, 10>>,
    )>,
    #[cfg(not(feature = "no-flash"))]
    flash_reset_pin: PD11<Output<PushPull>>,

    led_pin: PC10<Output<PushPull>>,
    scb: cortex_m::peripheral::SCB,

    sensor_command_queue: Arc<freertos_rust::Queue<threads::sensor_processor::Command>>,
}

impl WorkMode<RecorderMode> for RecorderMode {
    fn new(p: cortex_m::Peripherals, dp: Peripherals) -> Self {
        use crate::config::GENERATOR_DISABLE_LVL;

        let mut rcc = dp.RCC.constrain();

        let ic = Arc::new(InterruptController::new(p.NVIC));
        let dma_channels = dp.DMA1.split(&mut rcc.ahb1);

        let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);
        let mut gpioc = dp.GPIOC.split(&mut rcc.ahb2);
        let mut gpiod = dp.GPIOD.split(&mut rcc.ahb2);

        #[cfg(not(feature = "no-flash"))]
        let (qspi, flash_reset_pin) = {
            let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);
            let mut gpioe = dp.GPIOE.split(&mut rcc.ahb2);

            super::common::create_qspi(
                (
                    gpioa
                        .pa3
                        .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl),
                    gpioa
                        .pa2
                        .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl),
                    gpioe
                        .pe12
                        .into_alternate(&mut gpioe.moder, &mut gpioe.otyper, &mut gpioe.afrh),
                    gpiob
                        .pb0
                        .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl),
                    gpioa
                        .pa7
                        .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl),
                    gpioa
                        .pa6
                        .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl),
                ),
                gpiod.pd11.into_push_pull_output_in_state(
                    &mut gpiod.moder,
                    &mut gpiod.otyper,
                    PinState::Low,
                ),
                &mut rcc.ahb3,
            )
        };

        RecorderMode {
            flash: Arc::new(Mutex::new(dp.FLASH.constrain()).unwrap()),
            crc: Arc::new(
                Mutex::new(super::configure_crc_module(dp.CRC.constrain(&mut rcc.ahb1))).unwrap(),
            ),

            pwr: dp.PWR.constrain(&mut rcc.apb1r1),
            clocks: None,

            interrupt_controller: ic,

            #[cfg(not(feature = "no-flash"))]
            qspi,
            #[cfg(not(feature = "no-flash"))]
            flash_reset_pin,

            rcc,

            in_p: gpioa
                .pa8
                .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh)
                .set_speed(Speed::Low),
            in_t: gpioa
                .pa0
                .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl)
                .set_speed(Speed::Low),

            en_p: gpiod
                .pd13
                .into_push_pull_output_in_state(
                    &mut gpiod.moder,
                    &mut gpiod.otyper,
                    GENERATOR_DISABLE_LVL,
                )
                .set_speed(Speed::Low),
            en_t: gpiod
                .pd10
                .into_push_pull_output_in_state(
                    &mut gpiod.moder,
                    &mut gpiod.otyper,
                    GENERATOR_DISABLE_LVL,
                )
                .set_speed(Speed::Low),

            dma1_ch2: dma_channels.2,
            dma1_ch6: dma_channels.6,
            timer1: dp.TIM1,
            timer2: dp.TIM2,

            adc: dp.ADC1,
            adc_common: dp.ADC_COMMON,
            vbat_pin: gpioa.pa1.into_analog(&mut gpioa.moder, &mut gpioa.pupdr),

            led_pin: gpioc
                .pc10
                .into_push_pull_output_in_state(
                    &mut gpioc.moder,
                    &mut gpioc.otyper,
                    crate::config::LED_DISABLE,
                )
                .set_speed(Speed::Low),
            scb: p.SCB,

            sensor_command_queue: Arc::new(freertos_rust::Queue::new(40).unwrap()),
        }
    }

    fn flash(&mut self) -> Arc<Mutex<stm32l4xx_hal::flash::Parts>> {
        self.flash.clone()
    }

    fn crc(&mut self) -> Arc<Mutex<stm32l4xx_hal::crc::Crc>> {
        self.crc.clone()
    }

    fn ini_static(&mut self) {
        crate::settings::init(self.flash(), self.crc());
    }

    // Работа от внешнего кварца HSE = 12 MHz
    // Установить частоту CPU = 12 MHz
    // USB не тактируется
    fn configure_clock(&mut self) {
        //let clocks = if let Ok(mut flash) = self.flash.lock(Duration::infinite()) {
        //    clocks_recorder_mode(
        //        &mut self.rcc,
        //        &mut flash,
        //        &mut self.pwr,
        //        crate::config::XTAL_FREQ,
        //    )
        //} else {
        //    panic!()
        //};
        //
        //// stm32l433cc.pdf: fugure. 4
        //master_counter::MasterCounter::init(
        //    RecorderClockConfigProvider::master_counter_frequency(),
        //    self.interrupt_controller.clone(),
        //);
        //
        //self.clocks = Some(clocks);
    }

    fn start_threads(mut self) -> Result<(), freertos_rust::FreeRtosError> {
        let output = Arc::new(Mutex::new(OutputStorage::default()).unwrap());

        let sys_clk = unsafe { self.clocks.unwrap_unchecked().hclk() };

        #[cfg(not(feature = "no-flash"))]
        crate::main_data_storage::init(self.qspi, sys_clk, self.flash_reset_pin);

        {
            use stm32l4xx_hal::adc::{Resolution, SampleTime};

            defmt::trace!("Creating Sensors Processor thread...");
            let mut delay = FreeRtosDelay {};
            {
                // Enable peripheral
                stm32::ADC1::enable(&mut self.rcc.ahb2);

                // Reset peripheral
                stm32::ADC1::reset(&mut self.rcc.ahb2);

                self.adc_common
                    .ccr
                    .modify(|_, w| unsafe { w.presc().bits(0b0100) });
            }
            let mut adc = ADC::new(
                self.adc,
                self.adc_common,
                &mut self.rcc.ahb2,
                &mut self.rcc.ccipr,
                &mut delay,
            );

            adc.set_sample_time(SampleTime::Cycles640_5);
            adc.set_resolution(Resolution::Bits12);

            #[cfg(not(feature = "no-flash"))]
            {
                let tcpu_ch = adc.enable_temperature(&mut delay);
                let v_ref = adc.enable_vref(&mut delay);
                let sp = threads::sensor_processor::SensorPerith {
                    timer1: self.timer1,
                    timer1_dma_ch: self.dma1_ch6,
                    timer1_pin: self.in_p,
                    en_1: self.en_p,

                    timer2: self.timer2,
                    timer2_dma_ch: self.dma1_ch2,
                    timer2_pin: self.in_t,
                    en_2: self.en_t,

                    vbat_pin: self.vbat_pin,
                    tcpu_ch: tcpu_ch,
                    v_ref: v_ref,

                    adc: adc,
                };
                let cq = self.sensor_command_queue.clone();
                let ic = self.interrupt_controller.clone();
                //let mut processor = RecorderProcessor::new(
                //    output.clone(),
                //    self.sensor_command_queue.clone(),
                //    RecorderClockConfigProvider::xtal2master_freq_multiplier(),
                //    sys_clk,
                //);

                //processor.start(
                //    self.scb,
                //    crate::main_data_storage::diff_writer::FlashDiffWriter::new(
                //        RecorderClockConfigProvider::xtal2master_freq_multiplier() as f32,
                //        self.crc.clone(),
                //    ),
                //    self.led_pin,
                //)?;

                //Task::new()
                //    .name("SensProc")
                //    .stack_size(1024)
                //    .priority(TaskPriority(crate::config::SENS_PROC_TASK_PRIO))
                //    .start(move |_| {
                //        threads::sensor_processor::sensor_processor(sp, cq, ic, processor, sys_clk)
                //    })?;
            }
        }
        // --------------------------------------------------------------------

        crate::workmodes::common::create_monitor(sys_clk, output.clone())?;

        //super::common::create_pseudo_idle_task()?;

        Ok(())
    }

    fn print_clock_config(&self) {
        super::common::print_clock_config(&self.clocks, "OFF");
    }
}

pub struct RecorderClockConfigProvider<
    const XTAL_FREQ: u32,
    const CPU_FREQ: u32,
    const APB1_DEVIDER: u32 = 1,
    const APB2_DEVIDER: u32 = 1,
>;

impl<
        const XTAL_FREQ: u32,
        const CPU_FREQ: u32,
        const APB1_DEVIDER: u32,
        const APB2_DEVIDER: u32,
    > ClockConfigProvider
    for RecorderClockConfigProvider<XTAL_FREQ, CPU_FREQ, APB1_DEVIDER, APB2_DEVIDER>
{
    fn core_frequency() -> Hertz {
        Hertz(CPU_FREQ)
    }

    fn apb1_frequency() -> Hertz {
        Hertz(Self::core_frequency().0 / APB1_DEVIDER)
    }

    fn apb2_frequency() -> Hertz {
        Hertz(Self::core_frequency().0 / APB2_DEVIDER)
    }

    // stm32_cube: if APB devider > 1, timers freq APB*2
    fn master_counter_frequency() -> Hertz {
        if APB1_DEVIDER > 1 {
            Hertz(Self::apb1_frequency().0 * 2)
        } else {
            Self::apb1_frequency()
        }
    }

    fn pll_config() -> PllConfig {
        unreachable!()
    }

    fn xtal2master_freq_multiplier() -> f64 {
        if APB1_DEVIDER > 1 {
            2.0 / (XTAL_FREQ as f64 / CPU_FREQ as f64)
        } else {
            1.0 / (XTAL_FREQ as f64 / CPU_FREQ as f64)
        }
    }

    fn configure_clocks(
        flash: &mut stm32l4xx_hal::flash::Parts,
        rcc: &mut stm32l4xx_hal::rcc::Rcc,
        pwr: &mut stm32l4xx_hal::pwr::Pwr,
    ) -> stm32l4xx_hal::rcc::Clocks {
        let clocks = MyCFGR::new()
            .hse(
                Hertz(XTAL_FREQ),
                stm32l4xx_hal::rcc::CrystalBypass::Disable,
                stm32l4xx_hal::rcc::ClockSecuritySystem::Enable,
            )
            .sysclk(Hertz(CPU_FREQ))
            .hclk(Self::core_frequency())
            .pclk1(Self::apb1_frequency())
            .pclk2(Self::apb2_frequency())
            .freeze(&mut flash.acr, pwr);

        // low power run (F <= 2MHz) (на 12 MHz выйгрыш около 200мкА)
        unsafe {
            (*stm32l4xx_hal::device::PWR::ptr())
                .cr1
                .modify(|_, w| w.lpr().set_bit())
        };

        clocks
    }
}
