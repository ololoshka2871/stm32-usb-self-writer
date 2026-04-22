use core::{marker::PhantomData, pin::pin};
use futures::future;
use rtic_monotonics::{Monotonic, TimeoutError};

use no_std_async::Condvar;

use crate::config;

pub struct RtcSync<M: Monotonic<Duration = config::Duration>> {
    rtc_period: M::Duration,
    rtc_event: Condvar,
    _monotonic: PhantomData<M>,
}

impl<M: Monotonic<Duration = config::Duration>> RtcSync<M> {
    pub fn new(rtc_period: M::Duration) -> Self {
        Self {
            rtc_period,
            rtc_event: Condvar::new(),
            _monotonic: PhantomData,
        }
    }

    pub async fn delay_sync(&self, duration: M::Duration) {
        let now = M::now();
        self.rtc_event.wait().await;
        let elapsed = M::now() - now;

        if duration > elapsed {
            M::delay(duration - elapsed).await;
        }
    }

    pub async fn delay_until_sync(&self, deadline: M::Instant) {
        self.rtc_event.wait().await;
        if deadline > M::now() {
            M::delay_until(deadline).await;
        }
    }

    pub fn until_deadline_millis(&self, deadline: M::Instant) -> u32 {
        let now = M::now();
        if deadline > now {
            (deadline - now).to_millis()
        } else {
            0
        }
    }

    pub fn make_measure_time(&self, deadline: M::Instant) -> M::Duration {
        let now = M::now();
        if deadline > now {
            let until_deadline = deadline - now;
            let zapas = M::Duration::millis(config::MAKE_MEASURE_TIME_ZAPAS_MS);

            return if until_deadline > self.rtc_period {
                let a = until_deadline.ticks() / self.rtc_period.ticks();
                self.rtc_period * a as u32 - zapas
            } else if until_deadline > zapas {
                until_deadline - zapas
            } else {
                until_deadline
            };
        }
        M::Duration::from_ticks(0)
    }

    pub async fn timeout_after<F: core::future::Future>(
        &self,
        duration: M::Duration,
        future: F,
    ) -> Result<F::Output, TimeoutError> {
        let timeout = pin!(self.delay_sync(duration));
        let future = pin!(future);
        match future::select(timeout, future).await {
            future::Either::Left((_timeout, _)) => Err(TimeoutError),
            future::Either::Right((res, _)) => Ok(res),
        }
    }

    pub async fn timeout_at<F: core::future::Future>(
        &self,
        deadline: M::Instant,
        future: F,
    ) -> Result<F::Output, TimeoutError> {
        let timeout = pin!(self.delay_until_sync(deadline));
        let future = pin!(future);
        match future::select(timeout, future).await {
            future::Either::Left((_timeout, _)) => Err(TimeoutError),
            future::Either::Right((res, _)) => Ok(res),
        }
    }

    pub fn notify_all(&self) {
        self.rtc_event.notify_all();
    }
}
