use flash_settings_rs::StoragePolicy;
use stm32l4xx_hal::flash::{self, WriteErase};

use crate::support::crc::ZlibCompantCrc32;

pub struct Placeholder<T> {
    _body: T,
    _crc: u64,
}

/// https://docs.rs/stm32l4xx-hal/0.6.0/stm32l4xx_hal/flash/index.html
pub struct FlasRWPolcy<T: Sized, CRC: ZlibCompantCrc32> {
    flash: stm32l4xx_hal::flash::Parts,
    crc: CRC,
    page: flash::FlashPage,
    _phantom: core::marker::PhantomData<T>,
}

impl<T: Sized, CRC: ZlibCompantCrc32> FlasRWPolcy<T, CRC> {
    pub fn create(data: &Placeholder<T>, flash: stm32l4xx_hal::flash::Parts, crc: CRC) -> Self {
        const PAGE0_ADDR: usize = flash::FlashPage(0).to_address();
        const PAGE_SIZE: usize = flash::FlashPage(1).to_address() - PAGE0_ADDR;

        let addres = data as *const Placeholder<T> as usize;
        assert!(addres > PAGE0_ADDR);
        assert_eq!(addres % PAGE_SIZE, 0);

        Self {
            flash,
            crc,
            page: flash::FlashPage((addres - PAGE0_ADDR) / PAGE_SIZE),
            _phantom: core::marker::PhantomData,
        }
    }

    fn crc(&mut self, data: &[u8]) -> u32 {
        self.crc.reset();
        self.crc.feed(data);
        self.crc.result()
    }
}

impl<T: Sized, CRC: ZlibCompantCrc32> StoragePolicy<T, flash::Error> for FlasRWPolcy<T, CRC> {
    unsafe fn store_bytes(&mut self, data: &[u8]) -> Result<(), flash::Error> {
        let current_crc = [self.crc(data) as u64];

        let mut prog = self
            .flash
            .keyr
            .unlock_flash(&mut self.flash.sr, &mut self.flash.cr)?;

        let len_in_u64_aligned = crate::support::len_in_u64_aligned::len_in_u64_aligned(data);

        prog.erase_page(self.page)?;
        prog.write_native(
            self.page.to_address(),
            ::core::slice::from_raw_parts(data.as_ptr() as *const u64, len_in_u64_aligned),
        )?;

        prog.write_native(
            self.page.to_address() + len_in_u64_aligned * ::core::mem::size_of::<u64>(),
            &current_crc,
        )?;

        Ok(())
    }

    fn store(&mut self, v: &T) -> Result<(), flash::Error> {
        unsafe {
            self.store_bytes(core::slice::from_raw_parts(
                (v as *const T) as *const u8,
                core::mem::size_of::<T>(),
            ))
        }
    }

    unsafe fn load_bytes(
        &mut self,
        data: &mut [u8],
    ) -> Result<(), flash_settings_rs::LoadError<flash::Error>> {
        core::ptr::copy_nonoverlapping(
            self.page.to_address() as *const _,
            data.as_mut_ptr(),
            data.len(),
        );

        let len_aligned = crate::support::len_in_u64_aligned::len_in_u64_aligned(data)
            * ::core::mem::size_of::<u64>();
        let mut crc: u64 = core::mem::MaybeUninit::zeroed().assume_init();

        core::ptr::copy_nonoverlapping(
            (self.page.to_address() + len_aligned) as *const _,
            &mut crc,
            1,
        );

        if crc != self.crc(data) as u64 {
            Err(flash_settings_rs::LoadError::ConststenceError)
        } else {
            Ok(())
        }
    }

    fn load(&mut self) -> Result<T, flash_settings_rs::LoadError<flash::Error>> {
        let mut res = unsafe { core::mem::MaybeUninit::uninit().assume_init() };

        unsafe {
            self.load_bytes(core::slice::from_raw_parts_mut(
                (&mut res as *mut T) as *mut u8,
                core::mem::size_of::<T>(),
            ))
        }?;

        Ok(res)
    }
}
