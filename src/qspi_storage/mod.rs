pub mod flash_config;
mod identification;
pub mod qspi_driver;

use core::cell::RefCell;

use identification::Identification;
use qspi_stm32lx3::iqspi::IQspi;
use qspi_stm32lx3::qspi::Qspi;
use usbd_scsi::direct_read::DirectReadHack;

use alloc::{boxed::Box, sync::Arc};
use stm32l4xx_hal::traits::flash;

use qspi_driver::{FlashDriver, QSpiDriver};

//use super::PageAccessor;

use qspi_driver::QspiError;

use crate::config;
use crate::main_data_storage::{StorageDriver, StorageInfo};

const QSPI_MEMORY_MAPPED_REGION: *mut u8 = 0x90000000 as *mut u8;

//pub struct QSPIFlashPageAccessor {
//    driver: Arc<Mutex<Box<dyn FlashDriver + 'static>>>,
//    ptr: *mut u8,
//}

//impl PageAccessor for QSPIFlashPageAccessor {
//    fn write(&mut self, data: &[u8]) -> Result<(), flash::Error> {
//        if let Ok(mut guard) = self.driver.lock(Duration::infinite()) {
//            let addr24 = unsafe { self.ptr.sub(QSPI_MEMORY_MAPPED_REGION as usize) as u32 };
//            guard
//                .write_block(addr24, data)
//                .map_err(|_| flash::Error::Failure)
//        } else {
//            unreachable!()
//        }
//    }
//
//    fn read_to(&self, offset: usize, dest: &mut [u8]) {
//        if let Ok(mut guard) = self.driver.lock(Duration::infinite()) {
//            let addr24 =
//                unsafe { self.ptr.sub(QSPI_MEMORY_MAPPED_REGION as usize - offset) as u32 };
//            let _ = guard.read_direct(addr24, dest);
//
//            /*
//            guard.set_memory_mapping_mode(true).unwrap();
//
//            unsafe {
//                core::ptr::copy_nonoverlapping(self.ptr.add(offset), dest.as_mut_ptr(), dest.len())
//            };
//            */
//        } else {
//            unreachable!()
//        }
//    }
//
//    fn map_to_mem(&self, offset: usize) -> DirectReadHack {
//        if let Ok(mut guard) = self.driver.lock(Duration::infinite()) {
//            guard.set_memory_mapping_mode(true).unwrap();
//
//            DirectReadHack::new(unsafe { self.ptr.add(offset) })
//        } else {
//            unreachable!()
//        }
//    }
//
//    fn erase(&mut self) -> Result<(), flash::Error> {
//        if let Ok(mut guard) = self.driver.lock(Duration::zero()) {
//            guard.erase().map_err(|_| flash::Error::Failure)
//        } else {
//            Err(flash::Error::Busy)
//        }
//    }
//}

//impl Drop for QSPIFlashPageAccessor {
//    fn drop(&mut self) {
//        if let Ok(mut guard) = self.driver.lock(Duration::zero()) {
//            guard.want_sleep();
//        }
//    }
//}

pub struct QSPIStorage {
    driver: Arc<RefCell<Box<dyn FlashDriver + 'static>>>,
}

//impl super::storage::Storage<'static> for QSPIStorage {
//    fn select_page(&mut self, page: u32) -> Result<Box<dyn PageAccessor + 'static>, flash::Error> {
//        let full_adress = (page * self.flash_page_size()) as usize;
//        let addr24 = full_adress & 0x00FFFFFF;
//
//        if let Ok(mut guard) = self.driver.lock(Duration::infinite()) {
//            guard.wake_up().map_err(|_| flash::Error::Failure)?;
//            if let Err(_) = guard.set_addr_extender((full_adress >> 24) as u8) {
//                return Err(flash::Error::Failure);
//            }
//        }
//
//        let d: Box<dyn PageAccessor + 'static> = Box::new(QSPIFlashPageAccessor {
//            driver: self.driver.clone(),
//            ptr: unsafe { QSPI_MEMORY_MAPPED_REGION.add(addr24) },
//        });
//        Ok(d)
//    }
//
//    fn flash_erease(&mut self) -> Result<(), flash::Error> {
//        if let Ok(mut guard) = self.driver.lock(Duration::zero()) {
//            guard.erase().map_err(|_| flash::Error::Failure)
//        } else {
//            Err(flash::Error::Busy)
//        }
//    }
//
//    fn flash_size(&mut self) -> usize {
//        if let Ok(guard) = self.driver.lock(Duration::zero()) {
//            guard.get_capacity()
//        } else {
//            0
//        }
//    }
//
//    fn flash_size_pages(&mut self) -> u32 {
//        self.flash_size() as u32 / self.flash_page_size()
//    }
//
//    fn flash_page_size(&mut self) -> u32 {
//        // Запись ведется блоками по 256 байт, это буфер для сжатия, выгодно делать его
//        // как можно большим
//        4096
//    }
//}

impl QSPIStorage {
    pub fn new<QSPI, M>(
        qspi: QSPI,
        id: Identification,
        dual: bool,
        sys_clk: stm32l4xx_hal::time::Hertz,
    ) -> Result<Self, QspiError>
    where
        QSPI: IQspi + 'static,
        M: rtic_monotonics::Monotonic<Duration = config::Duration, Instant = config::Instant>
            + 'static,
    {
        QSpiDriver::<M>::init(Box::new(qspi), id, dual, sys_clk).map(|d| Self { driver: d })
    }
}

impl StorageDriver for QSPIStorage {
    fn make_info_accessor(&self, _driver: Arc<dyn StorageDriver>) -> Box<dyn StorageInfo> {
        todo!()
    }
}

//-----------------------------------------------------------------------------

pub fn probe(
    qspi: &mut dyn IQspi,
    sys_clk: stm32l4xx_hal::time::Hertz,
) -> Result<Identification, QspiError> {
    let config = qspi_stm32lx3::QspiConfig::default()
        /* failsafe config */
        .clock_prescaler((sys_clk.0 / 1_000_000) as u8)
        .clock_mode(qspi_stm32lx3::qspi::ClockMode::Mode3);

    qspi.apply_config(config);

    qspi_driver::get_jedec_id_cfg(qspi, false)
}
