#![no_std]

pub enum LoadError<T> {
    ReadError(T),
    ConststenceError,
}

pub trait StoragePolicy<T: Sized, E> {
    unsafe fn store_bytes(&mut self, data: &[u8]) -> Result<(), E>;
    fn store(&mut self, v: &T) -> Result<(), E>;
    unsafe fn load_bytes(&mut self, data: &mut [u8]) -> Result<(), LoadError<E>>;
    fn load(&mut self) -> Result<T, LoadError<E>>;
}

pub struct SettingsManager<T: 'static, U> {
    work_copy: T,
    non_store_values: U,
    default: &'static T,
}

impl<T, U> SettingsManager<T, U>
where
    T: Copy + Sized,
{
    pub fn load<Terr, Tpolicy: StoragePolicy<T, Terr>>(
        &mut self,
        policy: &mut Tpolicy,
    ) -> Result<T, LoadError<Terr>> {
        self.work_copy = policy.load()?;
        Ok(self.work_copy.clone())
    }

    pub fn save<Terr, Tpolicy: StoragePolicy<T, Terr>>(
        &self,
        policy: &mut Tpolicy,
    ) -> Result<(), Terr> {
        policy.store(&self.work_copy)
    }

    pub fn ref_mut(&mut self) -> (&mut T, &mut U) {
        (&mut self.work_copy, &mut self.non_store_values)
    }

    pub fn new<Terr, Tpolicy: StoragePolicy<T, Terr>>(
        default: &'static T,
        non_store_values_init: U,
        policy: &mut Tpolicy,
    ) -> Self {
        let mut res = Self {
            work_copy: unsafe { core::mem::MaybeUninit::uninit().assume_init() },
            non_store_values: non_store_values_init,
            default,
        };

        match res.load(policy) {
            Ok(_r) => res,
            Err(LoadError::<Terr>::ConststenceError) => {
                res.work_copy = *res.default;
                if res.save(policy).is_err() {
                    panic!("Failed to save default settings")
                }
                res
            }
            Err(LoadError::<Terr>::ReadError(_)) => panic!("Failed to init settings"),
        }
    }
}
