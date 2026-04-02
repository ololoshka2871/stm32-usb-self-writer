use alloc::sync::Arc;

#[allow(unused_imports)]
use freertos_rust::{Duration, Mutex, Task, TaskPriority};
use stm32l4xx_hal::{
    adc::ADC,
    gpio::{Input, PullUp},
    prelude::*,
    rcc::{Enable, PllConfig, Reset},
    stm32,
    stm32l4::stm32l4x3::Peripherals,
    time::Hertz,
};

#[allow(unused_imports)]
use stm32l4xx_hal::gpio::{
    Alternate, Analog, OpenDrain, Output, PushPull, Speed, PA0, PA1, PA2, PA3, PA6, PA7, PA8, PB0,
    PC0, PC1, PC10, PC2, PD10, PD11, PD13, PE12,
};

#[allow(unused_imports)]
use crate::{
    main_data_storage::{
        data_page::DataPage,
        write_controller::{PageWriteResult, WriteController},
    },
    sensors::freqmeter::master_counter,
    support::{interrupt_controller::IInterruptController, InterruptController},
    threads::{self, free_rtos_delay::FreeRtosDelay},
    workmodes::processing::RecorderProcessor,
};

use super::{common::ClockConfigProvider, output_storage::OutputStorage, WorkMode};

const APB1_DEVIDER: u32 = 1;
const APB2_DEVIDER: u32 = 1;

struct RecorderClockConfigProvider;

#[cfg(feature = "no-flash")]
struct NullDataPage {
    page_number: u32,
    samples: usize,
}

#[cfg(feature = "no-flash")]
impl DataPage for NullDataPage {
    fn write_header(&mut self, _output: &OutputStorage) {}

    fn push_data(&mut self, _result: Option<u32>, _channel: crate::threads::sensor_processor::FChannel) -> bool {
        self.samples += 1;
        self.samples >= 512
    }

    fn finalise(&mut self) {}
}

#[cfg(feature = "no-flash")]
struct NullWriteController;

#[cfg(feature = "no-flash")]
impl WriteController<NullDataPage> for NullWriteController {
    fn try_create_new_page(
        &mut self,
        page_number: u32,
    ) -> Result<NullDataPage, freertos_rust::FreeRtosError> {
        Ok(NullDataPage {
            page_number,
            samples: 0,
        })
    }

    fn write(&mut self, page: NullDataPage) -> PageWriteResult {
        PageWriteResult::Succes(page.page_number)
    }
}

impl ClockConfigProvider for RecorderClockConfigProvider {
    fn core_frequency() -> Hertz {
        Hertz(crate::config::FREERTOS_CONFIG_FREQ)
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
            2.0 / (crate::config::XTAL_FREQ as f64 / crate::config::FREERTOS_CONFIG_FREQ as f64)
        } else {
            1.0 / (crate::config::XTAL_FREQ as f64 / crate::config::FREERTOS_CONFIG_FREQ as f64)
        }
    }
}

#[derive(Debug, PartialEq)]
/// HSE Configuration
struct HseConfig {
    /// Clock speed of HSE
    speed: u32,
    /// If the clock driving circuitry is bypassed i.e. using an oscillator, not a crystal or
    /// resonator
    bypass: stm32l4xx_hal::rcc::CrystalBypass,
    /// Clock Security System enable/disable
    css: stm32l4xx_hal::rcc::ClockSecuritySystem,
}

struct MyCFGR {
    hse: HseConfig,
    hclk: Option<u32>,
    pclk1: Option<u32>,
    pclk2: Option<u32>,
    sysclk: u32,
}

impl MyCFGR {
    fn new() -> Self {
        Self {
            hse: HseConfig {
                speed: 0,
                bypass: stm32l4xx_hal::rcc::CrystalBypass::Disable,
                css: stm32l4xx_hal::rcc::ClockSecuritySystem::Enable,
            },
            hclk: None,
            pclk1: None,
            pclk2: None,
            sysclk: 0,
        }
    }

    /// Add an HSE to the system
    pub fn hse<F>(
        mut self,
        freq: F,
        bypass: stm32l4xx_hal::rcc::CrystalBypass,
        css: stm32l4xx_hal::rcc::ClockSecuritySystem,
    ) -> Self
    where
        F: Into<Hertz>,
    {
        self.hse = HseConfig {
            speed: freq.into().0,
            bypass,
            css,
        };

        self
    }

    /// Sets a frequency for the AHB bus
    pub fn hclk<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.hclk = Some(freq.into().0);
        self
    }

    /// Sets the system (core) frequency
    pub fn sysclk<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.sysclk = freq.into().0;
        self
    }

    /// Sets a frequency for the APB1 bus
    pub fn pclk1<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.pclk1 = Some(freq.into().0);
        self
    }

    /// Sets a frequency for the APB2 bus
    pub fn pclk2<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.pclk2 = Some(freq.into().0);
        self
    }

    fn freeze(
        &self,
        _acr: &mut stm32l4xx_hal::flash::ACR,
        _pwr: &mut stm32l4xx_hal::pwr::Pwr,
    ) -> stm32l4xx_hal::rcc::Clocks {
        // Поскольку поля stm32l4xx_hal::rcc::Clocks приватные, делает точно такую же
        // структуру, заполняем её и трансмутируем тип core::mem::transmute()
        #[derive(Clone, Copy, Debug)]
        #[allow(dead_code)]
        struct Clocks {
            hclk: Hertz,
            hsi48: bool,
            msi: Option<stm32l4xx_hal::rcc::MsiFreq>,
            lsi: bool,
            lse: bool,
            pclk1: Hertz,
            pclk2: Hertz,
            ppre1: u8,
            ppre2: u8,
            sysclk: Hertz,
            pll_source: Option<stm32l4xx_hal::rcc::PllSource>,
        }

        let rcc = unsafe { &*stm32::RCC::ptr() };
        //
        // 1. Setup clocks
        //

        // If HSE is available, set it up

        rcc.cr.write(|w| {
            w.hseon().set_bit();

            if self.hse.bypass == stm32l4xx_hal::rcc::CrystalBypass::Enable {
                w.hsebyp().set_bit();
            }

            w
        });

        while rcc.cr.read().hserdy().bit_is_clear() {}

        // Setup CSS
        if self.hse.css == stm32l4xx_hal::rcc::ClockSecuritySystem::Enable {
            // Enable CSS
            rcc.cr.modify(|_, w| w.csson().set_bit());
        }

        assert!(self.sysclk <= 80_000_000);

        let (hpre_bits, hpre_div) = self
            .hclk
            .map(|hclk| match self.sysclk / hclk {
                // From p 194 in RM0394
                0 => unreachable!(),
                1 => (0b0000, 1),
                2 => (0b1000, 2),
                3..=5 => (0b1001, 4),
                6..=11 => (0b1010, 8),
                12..=39 => (0b1011, 16),
                40..=95 => (0b1100, 64),
                96..=191 => (0b1101, 128),
                192..=383 => (0b1110, 256),
                _ => (0b1111, 512),
            })
            .unwrap_or((0b0000, 1));

        let hclk = self.sysclk / hpre_div;

        assert!(hclk <= self.sysclk);

        let (ppre1_bits, ppre1) = self
            .pclk1
            .map(|pclk1| match hclk / pclk1 {
                // From p 194 in RM0394
                0 => unreachable!(),
                1 => (0b000, 1),
                2 => (0b100, 2),
                3..=5 => (0b101, 4),
                6..=11 => (0b110, 8),
                _ => (0b111, 16),
            })
            .unwrap_or((0b000, 1));

        let pclk1 = hclk / ppre1 as u32;

        assert!(pclk1 <= self.sysclk);

        let (ppre2_bits, ppre2) = self
            .pclk2
            .map(|pclk2| match hclk / pclk2 {
                // From p 194 in RM0394
                0 => unreachable!(),
                1 => (0b000, 1),
                2 => (0b100, 2),
                3..=5 => (0b101, 4),
                6..=11 => (0b110, 8),
                _ => (0b111, 16),
            })
            .unwrap_or((0b000, 1));

        let pclk2 = hclk / ppre2 as u32;

        assert!(pclk2 <= self.sysclk);

        // adjust flash wait states
        unsafe {
            (*stm32::FLASH::ptr()).acr.write(|w| {
                w.latency().bits(if hclk <= 16_000_000 {
                    0b000
                } else if hclk <= 32_000_000 {
                    0b001
                } else if hclk <= 48_000_000 {
                    0b010
                } else if hclk <= 64_000_000 {
                    0b011
                } else {
                    0b100
                })
            })
        }

        let sysclk_src_bits = 0b10; // HSE

        // HSE: HSE selected as system clock
        rcc.cfgr.write(|w| unsafe {
            w.ppre2()
                .bits(ppre2_bits)
                .ppre1()
                .bits(ppre1_bits)
                .hpre()
                .bits(hpre_bits)
                .sw()
                .bits(sysclk_src_bits)
        });

        while rcc.cfgr.read().sws().bits() != sysclk_src_bits {}

        //
        // 3. Shutdown unused clocks that have auto-started
        //

        // MSI always starts on reset
        {
            rcc.cr
                .modify(|_, w| w.msion().clear_bit().msipllen().clear_bit())
        }

        //
        // 4. Clock setup done!
        //

        unsafe {
            core::mem::transmute(Clocks {
                hclk: Hertz(hclk),
                lsi: false,
                lse: false,
                msi: None,
                hsi48: false,
                pclk1: Hertz(pclk1),
                pclk2: Hertz(pclk2),
                ppre1,
                ppre2,
                sysclk: Hertz(self.sysclk),
                pll_source: None,
            })
        }
    }
}

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
    qspi: super::Flash,
    #[cfg(not(feature = "no-flash"))]
    flash_reset_pin: PD11<Output<PushPull>>,

    led_pin: PC10<Output<PushPull>>,
    tp1: super::TP1,
    rtc_scl: Option<PC0<Alternate<OpenDrain, 4>>>,
    rtc_sda: Option<PC1<Alternate<OpenDrain, 4>>>,
    rtc_1hz: Option<PC2<Input<PullUp>>>,
    i2c3: Option<stm32l4xx_hal::stm32::I2C3>,
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

        // Reserve PC0 as SCL and PC1 as SDA for I2C3 (open-drain) and PC2 for 1Hz input
        let mut rtc_scl_pin = gpioc.pc0.into_alternate_open_drain(
            &mut gpioc.moder,
            &mut gpioc.otyper,
            &mut gpioc.afrl,
        );
        rtc_scl_pin.internal_pull_up(&mut gpioc.pupdr, true); // enable internal pull-up for I2C lines

        let mut rtc_sda_pin = gpioc.pc1.into_alternate_open_drain(
            &mut gpioc.moder,
            &mut gpioc.otyper,
            &mut gpioc.afrl,
        );
        rtc_sda_pin.internal_pull_up(&mut gpioc.pupdr, true); // enable internal pull-up for I2C lines

        let rtc_1hz_pin = gpioc
            .pc2
            .into_pull_up_input(&mut gpioc.moder, &mut gpioc.pupdr);

        #[cfg(not(feature = "no-flash"))]
        let (qspi, flash_reset_pin) = {
            let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);
            #[allow(unused)]
            let mut gpioe = dp.GPIOE.split(&mut rcc.ahb2);

            #[cfg(feature = "maket")]
            let d0pin =
                gpioe
                    .pe12
                    .into_alternate(&mut gpioe.moder, &mut gpioe.otyper, &mut gpioe.afrh);
            #[cfg(not(feature = "maket"))]
            let d0pin =
                gpiob
                    .pb1
                    .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);

            super::common::create_qspi(
                (
                    gpioa
                        .pa3
                        .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl),
                    gpioa
                        .pa2
                        .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl),
                    d0pin,
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

        #[cfg(feature = "maket")]
        let tp1 = gpiod
            .pd3
            .into_push_pull_output_in_state(&mut gpiod.moder, &mut gpiod.otyper, PinState::Low)
            .set_speed(Speed::Low);

        #[cfg(not(feature = "maket"))]
        let tp1 = gpiod
            .pd0
            .into_push_pull_output_in_state(&mut gpiod.moder, &mut gpiod.otyper, PinState::Low)
            .set_speed(Speed::Low);

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
                    GENERATOR_DISABLE_LVL.into(),
                )
                .set_speed(Speed::Low),
            en_t: gpiod
                .pd10
                .into_push_pull_output_in_state(
                    &mut gpiod.moder,
                    &mut gpiod.otyper,
                    GENERATOR_DISABLE_LVL.into(),
                )
                .set_speed(Speed::Low),

            dma1_ch2: dma_channels.2,
            dma1_ch6: dma_channels.6,
            timer1: dp.TIM1,
            timer2: dp.TIM2,

            adc: dp.ADC1,
            adc_common: dp.ADC_COMMON,
            vbat_pin: gpioa.pa1.into_analog(&mut gpioa.moder, &mut gpioa.pupdr),

            rtc_sda: Some(rtc_sda_pin),
            rtc_scl: Some(rtc_scl_pin),
            rtc_1hz: Some(rtc_1hz_pin),
            i2c3: Some(dp.I2C3),

            led_pin: gpioc
                .pc10
                .into_push_pull_output_in_state(
                    &mut gpioc.moder,
                    &mut gpioc.otyper,
                    crate::config::LED_DISABLE.into(),
                )
                .set_speed(Speed::Low),
            scb: p.SCB,

            tp1,

            sensor_command_queue: Arc::new(freertos_rust::Queue::new(64).unwrap()),
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
        fn setup_cfgr() -> MyCFGR {
            MyCFGR::new()
                // TODO: constants
                .hse(
                    Hertz(crate::config::XTAL_FREQ), // onboard crystall
                    stm32l4xx_hal::rcc::CrystalBypass::Disable,
                    stm32l4xx_hal::rcc::ClockSecuritySystem::Enable,
                )
                .sysclk(Hertz(crate::config::XTAL_FREQ))
                .hclk(RecorderClockConfigProvider::core_frequency())
                .pclk1(RecorderClockConfigProvider::apb1_frequency())
                .pclk2(RecorderClockConfigProvider::apb2_frequency())
        }

        let cfgr = setup_cfgr();

        let clocks = if let Ok(mut flash) = self.flash.lock(Duration::infinite()) {
            cfgr.freeze(&mut flash.acr, &mut self.pwr)
        } else {
            panic!()
        };

        // low power run (F <= 2MHz) (на 12 MHz выйгрыш около 200мкА)
        unsafe {
            (*stm32l4xx_hal::device::PWR::ptr())
                .cr1
                .modify(|_, w| w.lpr().set_bit())
        };

        // stm32l433cc.pdf: fugure. 4
        master_counter::MasterCounter::init(
            RecorderClockConfigProvider::master_counter_frequency(),
            self.interrupt_controller.clone(),
        );

        self.clocks = Some(clocks);
    }

    fn start_threads(mut self) -> Result<(), freertos_rust::FreeRtosError> {
        let output = Arc::new(Mutex::new(OutputStorage::default()).unwrap());
        let sys_clk = unsafe { self.clocks.unwrap_unchecked().hclk() };

        // Initialize RTC and enable EXTI via shared helper.
        {
            let apb1r1 = &mut self.rcc.apb1r1;
            let pins = (
                self.rtc_scl.take(),
                self.rtc_sda.take(),
                self.i2c3.take(),
                self.clocks.take(),
            );
            crate::workmodes::common::init_rtc_with(move || {
                use crate::workmodes::common::new_i2c_config;
                use stm32l4xx_hal::i2c::I2c;

                if let (Some(scl), Some(sda), Some(i2c_per), Some(clocks)) = pins {
                    let config = new_i2c_config(clocks);
                    Ok(I2c::i2c3(i2c_per, (scl, sda), config, apb1r1))
                } else {
                    Err(freertos_rust::FreeRtosError::ProcessorHasShutDown)
                }
            })?;
        }

        let time = crate::rtc::rtc_get_time();
        defmt::info!(
            "RTC time: {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            time.year,
            time.month,
            time.day,
            time.hour,
            time.minute,
            time.second
        );

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
            let mut processor = RecorderProcessor::new(
                output.clone(),
                self.sensor_command_queue.clone(),
                RecorderClockConfigProvider::xtal2master_freq_multiplier(),
                sys_clk,
            );

            #[cfg(not(feature = "no-flash"))]
            processor.start(
                self.scb,
                crate::main_data_storage::diff_writer::FlashDiffWriter::new(
                    RecorderClockConfigProvider::xtal2master_freq_multiplier() as f32,
                    self.crc.clone(),
                ),
                self.led_pin,
            )?;

            #[cfg(feature = "no-flash")]
            processor.start(self.scb, NullWriteController, self.led_pin)?;

            Task::new()
                .name("SensProc")
                .stack_size(1024)
                .priority(TaskPriority(crate::config::SENS_PROC_TASK_PRIO))
                .start(move |_| {
                    threads::sensor_processor::sensor_processor(sp, cq, ic, processor, sys_clk)
                })?;
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
