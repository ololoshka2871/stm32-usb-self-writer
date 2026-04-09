use rtic_monotonics::{fugit::ExtU64, Monotonic};
use rtic_sync::channel::Receiver;

use crate::{config, sensors::freqmeter::Capture};

#[derive(Clone, Copy, PartialEq, defmt::Format)]
pub enum FreqmeterStates<M: Monotonic> {
    /// Генераторы отключены, входные счетчики остановлены
    /// remaining - время, сколько осталось до перехода в следующее состояние
    PowerOff { remaining: M::Duration },
    /// Генераторы включены и прогреваются, входные счетчики остановлены
    /// remaining - время, сколько осталось до перехода в следующее состояние
    /// не менее config::PREHEAT_MIN_MS миллисекунд, кратно базовому периоду
    Preheating { remaining: M::Duration },
    /// Переадаптация, если был сбой или пеерход из Preheating  
    /// занимает время config::INITIAL_FREQMETER_TARGET периодов измеряемой частоты
    Adaptation { remaining: M::Duration },
    /// Нормальная работа, генераторы включены, входные счетчики работают
    /// prev_freq - результат предыдущего измерения, не 0!
    /// measure_time - время, за которое должно быть сделано измерение, не менее config::BASE_INTERVAL_MIN_MS миллисекунд, кратно базовому периоду
    Measure {
        prev_freq: f32,
        remaining: M::Duration,
    },
}

impl<M: Monotonic<Duration = config::Duration>> FreqmeterStates<M> {
    pub const MIN_MEASURE_TIME: M::Duration =
        config::Duration::millis(config::BASE_INTERVAL_MIN_MS);
    pub const MAX_MEASURE_TIME: M::Duration = config::Duration::millis(config::MEASURE_TIME_MAX_MS);
    pub const ADAPTATION_TIME: M::Duration =
        config::Duration::millis(1_000 / config::SYST_TIMER_HZ as u64);
    pub const MIN_PREHEAT_TIME: M::Duration =
        config::Duration::millis(config::PREHEAT_MIN_MS as u64);
    pub const MAX_PREHEAT_TIME: M::Duration =
        config::Duration::millis(config::PREHEAT_MULTIPLIER as u64 * config::BASE_INTERVAL_MIN_MS);

    pub const MIN_WARM_STARTUP_TIME: M::Duration = config::Duration::millis(
        config::BASE_INTERVAL_MIN_MS + (1_000 / config::SYST_TIMER_HZ as u64),
    );
    pub const MIN_COLD_STARTUP_TIME: M::Duration = config::Duration::millis(
        config::PREHEAT_MIN_MS as u64
            + config::BASE_INTERVAL_MIN_MS
            + (1_000 / config::SYST_TIMER_HZ as u64),
    );

    pub fn init(startup_delay: config::Duration) -> Self {
        if startup_delay > Self::MIN_COLD_STARTUP_TIME {
            Self::PowerOff {
                remaining: startup_delay - Self::MIN_COLD_STARTUP_TIME,
            }
        } else if startup_delay > Self::MAX_MEASURE_TIME {
            Self::Preheating {
                remaining: startup_delay - Self::MAX_MEASURE_TIME,
            }
        } else {
            Self::Adaptation {
                remaining: Self::ADAPTATION_TIME.min(startup_delay),
            }
        }
    }

    pub fn plan_next_state(
        base_period: M::Duration,
        base_period_devider: u32,
        prev_freq: f32,
    ) -> Self {
        let cycle_time = base_period * base_period_devider;

        if cycle_time > Self::MIN_COLD_STARTUP_TIME {
            Self::PowerOff {
                remaining: cycle_time - Self::MIN_COLD_STARTUP_TIME,
            }
        } else if cycle_time > Self::MAX_MEASURE_TIME {
            Self::Preheating {
                remaining: cycle_time - Self::MAX_MEASURE_TIME,
            }
        } else {
            Self::Measure {
                prev_freq,
                remaining: cycle_time,
            }
        }
    }

    pub fn adaptation(remaining: M::Duration, elapsed: M::Duration) -> Self {
        Self::Adaptation {
            remaining: if remaining > elapsed {
                (remaining - elapsed).max(Self::MIN_MEASURE_TIME)
            } else {
                Self::ADAPTATION_TIME
            },
        }
    }

    pub fn emergency_adaptation() -> Self {
        Self::Adaptation {
            remaining: Self::ADAPTATION_TIME,
        }
    }
}
