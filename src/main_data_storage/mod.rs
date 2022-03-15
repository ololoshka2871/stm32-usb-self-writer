pub mod data_page;
pub mod diff_writer;
pub mod write_controller;

pub mod storage;

pub(crate) mod header_printer;
mod qspi_storage;

use core::{marker::PhantomData, sync::atomic::AtomicBool};
use lazy_static::lazy_static;

use alloc::{boxed::Box, vec::Vec};
use freertos_rust::{Duration, DurationTicks, FreeRtosError, Mutex, Task, TaskPriority};

use stm32l4xx_hal::traits::flash;

use qspi_storage::qspi_driver::{ClkPin, IO0Pin, IO1Pin, IO2Pin, IO3Pin, NCSPin, QUADSPI};

enum MemoryState {
    Undefined,
    PartialUsed(u32),
    FullUsed,
}

static mut STORAGE_IMPL: Option<Box<dyn storage::Storage + 'static>> = None;
static mut ERASE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

lazy_static! {
    static ref STORAGE_LOCK: Mutex<PhantomData<u8>> = Mutex::new(PhantomData).unwrap();
    static ref NEXT_EMPTY_PAGE: Mutex<MemoryState> = Mutex::new(MemoryState::Undefined).unwrap();
}

pub trait PageAccessor {
    fn write(&mut self, data: Vec<u8>) -> Result<(), flash::Error>;
    fn read_to(&self, offset: usize, dest: &mut [u8]);
    fn erase(&mut self) -> Result<(), flash::Error>;
}

pub fn is_erase_in_progress() -> bool {
    unsafe { ERASE_IN_PROGRESS.load(core::sync::atomic::Ordering::Relaxed) }
}

pub fn flash_erease() -> Result<(), FreeRtosError> {
    if !is_erase_in_progress() {
        unsafe { ERASE_IN_PROGRESS.store(true, core::sync::atomic::Ordering::Relaxed) };
        let _ = Task::new()
            .name("FlashClr")
            .priority(TaskPriority(crate::config::FLASH_CLEANER_PRIO))
            .start(move |_| {
                let _ = lock_storage(Duration::infinite(), |s| {
                    if let Some(s) = s.as_deref_mut() {
                        if let Err(e) = s.flash_erease() {
                            defmt::error!("Flash erase error: {}", defmt::Debug2Format(&e));
                        } else {
                            defmt::info!("Flash erased succesfilly");
                        }
                    } else {
                        defmt::error!("Erase canceled");
                    }
                });

                unsafe { ERASE_IN_PROGRESS.store(false, core::sync::atomic::Ordering::Relaxed) };
            })?;

        Ok(())
    } else {
        Err(FreeRtosError::QueueFull)
    }
}

pub fn find_next_empty_page(start: u32) -> Option<u32> {
    if let Ok(guard) = NEXT_EMPTY_PAGE.lock(Duration::ms(1)) {
        match *guard {
            MemoryState::PartialUsed(next_free_page) => {
                if start <= next_free_page {
                    return Some(next_free_page);
                }
            }
            MemoryState::FullUsed => return None,
            _ => {}
        }
    }

    if start < flash_size_pages() {
        for p in start..flash_size_pages() {
            let accessor = unsafe { select_page(p).unwrap_unchecked() };
            let mut header_blockchain: self_recorder_packet::DataPacketHeader =
                unsafe { core::mem::MaybeUninit::uninit().assume_init() };
            accessor.read_to(0, unsafe {
                core::slice::from_raw_parts_mut(
                    &mut header_blockchain as *mut _ as *mut u8,
                    core::mem::size_of_val(&header_blockchain),
                )
            });

            // признаком того, что флешка стерта является то, что там везде FF
            if core::cmp::min(
                header_blockchain.this_block_id,
                header_blockchain.prev_block_id,
            ) == u32::MAX
            {
                if let Ok(mut guard) = NEXT_EMPTY_PAGE.lock(Duration::zero()) {
                    *guard = MemoryState::PartialUsed(p);
                }
                return Some(p);
            }
        }
    }
    if let Ok(mut guard) = NEXT_EMPTY_PAGE.lock(Duration::zero()) {
        *guard = MemoryState::FullUsed;
    }
    None
}

pub fn select_page<'a>(page: u32) -> Result<Box<dyn PageAccessor + 'a>, flash::Error> {
    lock_storage(Duration::ms(1), |s| {
        if let Some(s) = s.as_deref_mut() {
            s.select_page(page)
        } else {
            Err(flash::Error::Illegal)
        }
    })
    .unwrap_or_else(|e| Err(e))
}

pub fn flash_page_size() -> u32 {
    let res = lock_storage(Duration::zero(), |s| {
        s.as_deref_mut().map_or(0, |s| s.flash_page_size())
    });

    if let Ok(page_size) = res {
        page_size
    } else {
        0
    }
}

pub fn flash_size() -> usize {
    let res = lock_storage(Duration::zero(), |s| {
        s.as_deref_mut().map_or(0, |s| s.flash_size())
    });

    if let Ok(size) = res {
        size
    } else {
        0
    }
}

pub fn flash_size_pages() -> u32 {
    let res = lock_storage(Duration::zero(), |s| {
        s.as_deref_mut().map_or(0, |s| s.flash_size_pages())
    });

    if let Ok(pages) = res {
        pages
    } else {
        0
    }
}

pub(crate) fn init<CLK, NCS, IO0, IO1, IO2, IO3>(
    qspi: qspi_stm32lx3::qspi::Qspi<(CLK, NCS, IO0, IO1, IO2, IO3)>,
    qspi_base_clock_speed: stm32l4xx_hal::time::Hertz,
) where
    CLK: ClkPin<QUADSPI> + 'static,
    NCS: NCSPin<QUADSPI> + 'static,
    IO0: IO0Pin<QUADSPI> + 'static,
    IO1: IO1Pin<QUADSPI> + 'static,
    IO2: IO2Pin<QUADSPI> + 'static,
    IO3: IO3Pin<QUADSPI> + 'static,
{
    if let Ok(s) = qspi_storage::QSPIStorage::<'static>::init(qspi, qspi_base_clock_speed) {
        if lock_storage(Duration::infinite(), |sph| {
            *sph = Some(Box::new(s));
        })
        .is_ok()
        {
            let next_free_page = find_next_empty_page(0);
            if let Some(next_free_page) = next_free_page {
                defmt::info!("Memory: {} pages used", next_free_page);
            } else {
                defmt::warn!("Memory full!");
            }
        } else {
            unreachable!();
        }
    } else {
        defmt::error!("Failed to initialise QSPI flash! storage blocked!");
    }
}

fn lock_storage<T, D, F>(duration: D, f: F) -> Result<T, flash::Error>
where
    D: DurationTicks,
    F: FnOnce(&mut Option<Box<dyn storage::Storage<'static> + 'static>>) -> T,
{
    if let Ok(_) = STORAGE_LOCK.lock(duration) {
        Ok(unsafe { f(&mut STORAGE_IMPL) })
    } else {
        Err(flash::Error::Busy)
    }
}
