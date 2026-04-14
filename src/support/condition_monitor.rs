#[derive(PartialEq, Eq, core::marker::ConstParamTy)]
pub enum Ordering {
    Greater,
    Less,
}

#[derive(Clone)]
pub struct ConditionMonitor<const O: Ordering> {
    current: u32,
    count_limit: u32,
}

impl<const O: Ordering> ConditionMonitor<O> {
    pub fn new(count_limit: u32) -> Self {
        Self {
            current: 0,
            count_limit,
        }
    }
}

impl<const O: Ordering> ConditionMonitor<O> {
    pub fn check<T: Into<f32>>(&mut self, current: T, limit: f32) -> bool {
        if limit.is_nan() {
            return false;
        }

        let cmp = if O == Ordering::Greater {
            current.into() > limit
        } else {
            current.into() < limit
        };

        if cmp {
            if self.current < self.count_limit {
                self.current += 1;
                false
            } else {
                true
            }
        } else {
            self.current = 0;
            false
        }
    }
}
