pub struct SettingActionError<T>(pub T);

impl<T> SettingActionError<T> {
    pub fn new(e: T) -> Self {
        SettingActionError(e)
    }
}

impl<T: core::fmt::Display> defmt::Format for SettingActionError<T> {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "Setting action error: {}",
            defmt::Display2Format(&self.0)
        )
    }
}
