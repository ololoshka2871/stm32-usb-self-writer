use stm32l4xx_hal::gpio::PinState;

//-----------------------------------------------------------------------------

#[cfg(feature = "xtal-24mhz")]
pub const XTAL_FREQ: u32 = 24_000_000;

#[cfg(feature = "xtal-12mhz")]
pub const XTAL_FREQ: u32 = 12_000_000;

pub const HIGH_PERF_CPU_FREQ: u32 = 80_000_000;
pub const SELF_WRITER_CPU_FREQ: u32 = 3_000_000;
pub const HW_VERSION: u32 = 1;
pub const SYST_TIMER_HZ: u32 = 1_000;

pub type Duration = rtic_monotonics::fugit::Duration<u32, 1, { SYST_TIMER_HZ }>;
pub type Instant = rtic_monotonics::fugit::Instant<u32, 1, { SYST_TIMER_HZ }>;

//-----------------------------------------------------------------------------

pub const HEAP_SIZE: usize = 1024 * 8;

//-----------------------------------------------------------------------------

pub const INITIAL_FREQMETER_TARGET: u16 = 2;

//-----------------------------------------------------------------------------

// generator enable/disable lvls
pub const GENERATOR_ENABLE_LVL: PinState = PinState::High;
pub const GENERATOR_DISABLE_LVL: PinState = PinState::Low;

// Led
pub const LED_DISABLE: PinState = PinState::High;
pub const LED_ENABLE: PinState = PinState::Low;

//-----------------------------------------------------------------------------

pub const BULK_MAX_PACKET_SIZE: usize = 64;

//-----------------------------------------------------------------------------

pub const BASE_INTERVAL_MIN_MS: u32 = 20;

//-----------------------------------------------------------------------------

pub const MINIMUM_ADAPTATION_INTERVAL: u32 = 50;
pub const MEASURE_TIME_TO_GUARD_MULTIPLIER: f32 = 1.5;
pub const MIN_GUARD_TIME: f64 = 100.0;
pub const MEASURE_TIME_MAX_MS: u32 = 1000;
/// Запас на которое время измерения меньше дедлайна, из него вычисляется цель
/// Если используется DEFMT_LOG = "trace", увеличить до 10 в режиме самописца!
pub const MAKE_MEASURE_TIME_ZAPAS_MS: u32 = 5;

//-----------------------------------------------------------------------------

pub const OVER_LIMIT_COUNT: u32 = 5;

//-----------------------------------------------------------------------------

pub const VBAT_DEVIDER_R1: f32 = 270_000.0;
pub const VBAT_DEVIDER_R2: f32 = 91_000.0;

//-----------------------------------------------------------------------------

pub const START_BLINK_COUNT: u32 = 5;
pub const START_BLINK_PERIOD_MS: u32 = 500;

//-----------------------------------------------------------------------------

// включать счетчики за 2 периода измерения
pub const PREHEAT_MULTIPLIER: u32 = 2;
pub const PREHEAT_MIN_MS: u32 = 500;

// Счетчик, отскрочки включения частотомера после включения питания
pub const F_CH_START_COUNT: u32 = 2;

// Экспериментальное значение, время с подачи питания до запуска генератора
// Канал температуры дает 100kHz как минимум перые 100 ms, берем запас
pub const GEN_COLD_STARTUP_TIME_MS: u32 = 200;

//-----------------------------------------------------------------------------

// Задержка перехода флешки с спящий режим при неактивности
pub const FLASH_AUTO_POWER_DOWN_MS: u32 = 10;

//-----------------------------------------------------------------------------

pub const STORAGE_BLOCK_SIZE_BYTES: u32 = 4096;