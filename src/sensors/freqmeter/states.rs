use rtic_monotonics::Monotonic;

use crate::config;

#[derive(Clone, Copy, PartialEq, defmt::Format)]
pub enum FreqmeterStates<M: Monotonic> {
    /// Генераторы отключены, входные счетчики остановлены
    PowerOff { deadline: M::Instant },
    /// Генераторы включены и прогреваются, входные счетчики остановлены
    Preheating { deadline: M::Instant },
    /// Переадаптация, если был сбой или пеерход из Preheating  
    Adaptation { deadline: M::Instant },
    /// Нормальная работа, генераторы включены, входные счетчики работают
    Measure {
        prev_freq: f32,
        deadline: M::Instant,
    },
}

impl<M: Monotonic<Duration = config::Duration>> FreqmeterStates<M> {
    pub const MIN_MEASURE_TIME: M::Duration =
        config::Duration::millis(config::BASE_INTERVAL_MIN_MS);
    pub const MAX_MEASURE_TIME: M::Duration = config::Duration::millis(config::MEASURE_TIME_MAX_MS);
    pub const ADAPTATION_TIME: M::Duration =
        config::Duration::millis(1_000 / config::SYST_TIMER_HZ);
    pub const MIN_PREHEAT_TIME: M::Duration =
        config::Duration::millis(config::PREHEAT_MIN_MS);
    pub const MAX_PREHEAT_TIME: M::Duration =
        config::Duration::millis(config::PREHEAT_MULTIPLIER * config::BASE_INTERVAL_MIN_MS);

    pub const MIN_WARM_STARTUP_TIME: M::Duration = config::Duration::millis(
        config::BASE_INTERVAL_MIN_MS + (1_000 / config::SYST_TIMER_HZ),
    );
    pub const MIN_COLD_STARTUP_TIME: M::Duration = config::Duration::millis(
        config::PREHEAT_MIN_MS
            + config::BASE_INTERVAL_MIN_MS
            + (1_000 / config::SYST_TIMER_HZ),
    );

    pub fn init(startup_delay: config::Duration) -> Self {
        if startup_delay > Self::MIN_COLD_STARTUP_TIME {
            Self::PowerOff {
                deadline: M::now() + startup_delay - Self::MIN_COLD_STARTUP_TIME,
            }
        } else {
            Self::Preheating {
                deadline: M::now() + Self::MIN_COLD_STARTUP_TIME,
            }
        }
    }

    pub fn plan_next_state(
        prev_deadline: M::Instant,
        base_period: M::Duration,
        base_period_devider: u32,
        prev_freq: Option<f32>,
    ) -> Self {
        let cycle_time = base_period * base_period_devider;
        let next_deadline = prev_deadline + cycle_time;

        if cycle_time > Self::MIN_COLD_STARTUP_TIME {
            Self::PowerOff {
                deadline: next_deadline,
            }
        } else if cycle_time > Self::MAX_MEASURE_TIME {
            Self::Preheating {
                deadline: next_deadline,
            }
        } else if let Some(prev_freq) = prev_freq {
            Self::Measure {
                prev_freq,
                deadline: next_deadline,
            }
        } else {
            Self::Adaptation {
                deadline: next_deadline,
            }
        }
    }
}
