#![no_std]
#![no_main]

extern crate alloc;

use defmt_rtt as _; // global logger
use panic_abort as _;

use stm32l4xx_hal::{
    adc::{self, Resolution, SampleTime, Temperature, Vref, ADC},
    pac,
    prelude::*,
    serial,
};

use embedded_hal::serial::Write;

use rtic::app;
use rtic_monotonics::Monotonic;

use stm32_usb_self_writer::{
    clocking::rtc::{RtcCalibrationOutput, RtcTrimming, int_rtc::RtcService},
    config,
};

//-----------------------------------------------------------------------------

rtic_monotonics::systick_monotonic!(Mono, config::SYST_TIMER_HZ);

//-----------------------------------------------------------------------------

defmt::timestamp!(
    "[T{=u32:ms}]",
    Mono::now().ticks() * (1_000 / config::SYST_TIMER_HZ)
);

//-----------------------------------------------------------------------------

static mut HEAP: [u8; config::HEAP_SIZE] = [0; config::HEAP_SIZE];

//-----------------------------------------------------------------------------

#[app(device = stm32l4xx_hal::pac, peripherals = true, dispatchers = [RCC, LCD, TAMP_STAMP, SWPMI1])]
mod app {
    use super::*;

    #[shared]
    struct Shared {
        rtc: RtcService,
    }

    #[local]
    struct Local {
        uart3_rx: serial::Rx<pac::USART3>,
        uart3_tx: serial::Tx<pac::USART3>,
        adc: ADC,
        tcpu_ch: Temperature,
        v_ref: Vref,
    }

    fn uart_write<E, UART: Write<u8, Error = E>>(tx: &mut UART, data: &[u8]) -> Result<(), E> {
        for &b in data {
            while tx.write(b).is_err() {}
        }
        while tx.flush().is_err() {}
        Ok(())
    }

    fn uart_write_line<E, UART: Write<u8, Error = E>>(tx: &mut UART, text: &str) -> Result<(), E> {
        uart_write(tx, text.as_bytes())?;
        uart_write(tx, b"\r\n")?;
        Ok(())
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local) {
        let mut dp = ctx.device;

        #[cfg(feature = "force-defmt-logs")]
        // need for defmt logging works https://github.com/knurling-rs/probe-run/pull/183/files
        dp.RCC.ahb1enr.modify(|_, w| w.dma1en().set_bit());

        defmt::info!("+ Init +");

        ctx.core.DCB.enable_trace();
        ctx.core.DWT.enable_cycle_counter();
        defmt::info!("\tDWT");

        let mut flash = dp.FLASH.constrain();
        let mut rcc = dp.RCC.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);

        let clocks = rcc.cfgr.freeze(&mut flash.acr, &mut pwr);

        unsafe {
            #[allow(static_mut_refs)]
            umm_malloc::init_heap(HEAP.as_mut_ptr() as usize, config::HEAP_SIZE)
        };

        defmt::info!("\tHeap");

        // Initialize the systick interrupt & obtain the token to prove that we did
        Mono::start(ctx.core.SYST, clocks.hclk().to_Hz());
        defmt::info!("\tSysTick");

        let mut _gpiob = dp.GPIOB.split(&mut rcc.ahb2);
        let mut gpioc = dp.GPIOC.split(&mut rcc.ahb2);

        let (adc, tcpu_ch, v_ref) = {
            let mut delay = stm32_usb_self_writer::NOPDelay {
                sys_clk: clocks.sysclk(),
            };

            let mut adc = adc::ADC::new(
                dp.ADC1,
                dp.ADC_COMMON,
                &mut rcc.ahb2,
                &mut rcc.ccipr,
                &mut delay,
            );
            adc.set_sample_time(SampleTime::Cycles640_5);
            adc.set_resolution(Resolution::Bits12);

            let tcpu_ch = adc.enable_temperature(&mut delay);
            let v_ref = adc.enable_vref(&mut delay);

            (adc, tcpu_ch, v_ref)
        };

        defmt::info!("\tADC");

        let (uart3_rx, uart3_tx) = {
            let tx_pin =
                gpioc
                    .pc4
                    .into_alternate(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrl);
            let rx_pin =
                gpioc
                    .pc5
                    .into_alternate(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrl);

            let mut serial = serial::Serial::usart3(
                dp.USART3,
                (tx_pin, rx_pin),
                serial::Config::default().baudrate(115_200.bps()),
                clocks,
                &mut rcc.apb1r1,
            );
            serial.listen(serial::Event::Rxne);
            let (mut uart3_tx, uart3_rx) = serial.split();
            if let Err(err) = uart_write_line(&mut uart3_tx, "RTC trim ready") {
                defmt::error!("UART write error: {}", defmt::Debug2Format(&err));
            }

            (uart3_rx, uart3_tx)
        };

        defmt::info!("\tUART");

        let (mut rtc, cs) = stm32_usb_self_writer::clocking::rtc::int_rtc::RtcService::init(
            dp.RTC,
            &mut dp.EXTI,
            &mut rcc.apb1r1,
            &mut rcc.bdcr,
            &mut pwr.cr1,
        );
        rtc.set_alarm_period_ms(1_000);
        // Есть проблемы с pc13 - на макетке острые импульсы наводят помехи в генераторе,
        // поэтому оно либо генерит нестабильно, либо вообще не запускается. Поэтому для теста калибровки используем pb2, который по схеме подключен к тому же кварцу, что и pc13, так что частота будет одинаковой.
        rtc.enable_calibration_output(
            _gpiob
                .pb2
                .into_alternate_push_pull(&mut _gpiob.moder, &mut _gpiob.otyper, &mut _gpiob.afrl)
                .set_speed(stm32l4xx_hal::gpio::Speed::Low),
            //gpioc.pc13,
            stm32l4xx_hal::time::Hertz::Hz(512),
        )
        .unwrap();

        defmt::info!("\tRTC: {}", defmt::Debug2Format(&cs));

        //---------------------------------------------------------------------

        (
            Shared { rtc },
            Local {
                uart3_rx,
                uart3_tx,
                adc,
                tcpu_ch,
                v_ref,
            },
        )
    }

    //-------------------------------------------------------------------------

    #[task(
        binds = USART3,
        shared = [rtc],
        local = [uart3_rx, uart3_tx, uart_rx_buf: [u8; 64] = [0; 64], uart_rx_len: usize = 0, last_was_cr: bool = false]
    )]
    fn uart3(ctx: uart3::Context) {
        let uart_rx = ctx.local.uart3_rx;
        let uart_tx = ctx.local.uart3_tx;
        let rx_len = ctx.local.uart_rx_len;
        let rx_buf = ctx.local.uart_rx_buf;
        let last_was_cr = ctx.local.last_was_cr;

        let b = match uart_rx.read() {
            Ok(b) => b,
            Err(_) => {
                if let Err(err) = uart_write_line(uart_tx, "ERR uart rx") {
                    defmt::error!("UART write error: {}", defmt::Debug2Format(&err));
                }
                return;
            }
        };

        // Echo typed symbols for interactive terminal use.
        if b == b'\r' {
            uart_write(uart_tx, b"\r\n").ok();
            *last_was_cr = true;
        } else if b == b'\n' {
            if *last_was_cr {
                *last_was_cr = false;
                return;
            }

            uart_write(uart_tx, b"\r\n").ok();
        } else {
            *last_was_cr = false;
            uart_write(uart_tx, &[b]).ok();
        }

        if b == b'\r' || b == b'\n' {
            if *rx_len == 0 {
                return;
            }

            let line = &rx_buf[..*rx_len];
            *rx_len = 0;

            let result = core::str::from_utf8(line)
                .map(|s| s.trim())
                .map_err(|_| "ERR utf8")
                .and_then(|s| s.parse::<f32>().map_err(|_| "ERR parse f32"));

            match result {
                Ok(calibration_ppm) => {
                    let mut rtc = ctx.shared.rtc;
                    let apply_result = rtc.lock(|rtc| rtc.set_calibration(calibration_ppm));

                    let res = match apply_result {
                        Ok(()) => uart_write_line(uart_tx, "OK"),
                        Err(err) => uart_write_line(uart_tx, &alloc::format!("{:?}", err)),
                    };
                    if let Err(err) = res {
                        defmt::error!("UART write error: {}", defmt::Debug2Format(&err));
                    }
                }
                Err(err_text) => {
                    uart_write_line(uart_tx, err_text).ok();
                }
            }

            return;
        }

        if *rx_len < rx_buf.len() {
            rx_buf[*rx_len] = b;
            *rx_len += 1;
        } else {
            *rx_len = 0;
            uart_write_line(uart_tx, "ERR line too long").ok();
        }
    }

    #[task(
        binds = RTC_WKUP,
        shared = [rtc],
        local = [counter: u32 = 0, adc, tcpu_ch, v_ref],
        priority = 4
    )]
    fn rtc_alarm(ctx: rtc_alarm::Context) {
        const TRIMMING_COEFFS: [f32; 3] = [
            159.17595, // T^0
            1.31706, // T^1
            -0.03900,  // T^2
        ];

        let mut rtc = ctx.shared.rtc;
        let counter = ctx.local.counter;
        let adc = ctx.local.adc;
        let tcpu_ch = ctx.local.tcpu_ch;
        let v_ref = ctx.local.v_ref;

        let now = rtc.lock(|rtc| {
            rtc.handle_alarm_interrupt();
            rtc.current_time()
        });

        if *counter % 8 == 0 {
            adc.calibrate(v_ref);

            let temp_raw = adc.read(tcpu_ch).unwrap_or(0);
            let temp_c = adc.to_degrees_centigrade(temp_raw);

            let correction = TRIMMING_COEFFS[0] + temp_c * (TRIMMING_COEFFS[1] + temp_c * TRIMMING_COEFFS[2]);

            defmt::info!("CPU temp: {=f32} C (raw={=u16}), correction={}", temp_c, temp_raw, correction);

            // apply correction to RTC
            rtc.lock(|rtc| {
                if let Err(err) = rtc.set_calibration(correction) {
                    defmt::error!("Failed to set RTC calibration: {:?}", err);
                }
            });
        }
        *counter += 1;
    }
}
