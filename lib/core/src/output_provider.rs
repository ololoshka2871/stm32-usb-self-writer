/// Trait and error used to access an output storage provider from core code.
pub enum OutputProviderError<T> {
    LockFailed,
    Other(T),
}

pub trait OutputProvider {
    /// The concrete output storage type exposed by this provider.
    type Output;
    /// The error type produced when access fails.
    type Error;

    fn with_output<R>(&self, f: impl FnOnce(&Self::Output) -> R) -> Result<R, Self::Error>;
}
