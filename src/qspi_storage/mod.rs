pub mod flash_config;
mod identification;
pub mod qspi_driver;

use core::cell::RefCell;

use identification::Identification;
use qspi_stm32lx3::iqspi::IQspi;

use alloc::{boxed::Box, sync::Arc};
use cortex_m::interrupt::Mutex as InterruptMutex;

use qspi_driver::{FlashDriver, QSpiDriver};

use qspi_driver::QspiError;

use crate::config;
use crate::main_data_storage::{
    BlockMapper, FlashBanks, StorageDriver, StorageError, StorageGeometry, StorageInfo,
    StorageMetaHandle, install_meta_handle, install_read_range_fn,
};

type RuntimeDriver = Arc<RefCell<Box<dyn FlashDriver + 'static>>>;

#[derive(Clone)]
struct RuntimeReadAdapter {
    primary: RuntimeDriver,
    secondary: Option<RuntimeDriver>,
    mapper: BlockMapper,
}

struct RuntimeAdapterStore {
    inner: InterruptMutex<RefCell<Option<RuntimeReadAdapter>>>,
}

impl RuntimeAdapterStore {
    fn new() -> Self {
        Self {
            inner: InterruptMutex::new(RefCell::new(None)),
        }
    }

    fn replace(&self, adapter: RuntimeReadAdapter) {
        cortex_m::interrupt::free(|cs| {
            self.inner.borrow(cs).borrow_mut().replace(adapter);
        });
    }

    fn get_cloned(&self) -> Option<RuntimeReadAdapter> {
        cortex_m::interrupt::free(|cs| self.inner.borrow(cs).borrow().clone())
    }
}

unsafe impl Sync for RuntimeAdapterStore {}
unsafe impl Send for RuntimeAdapterStore {}

lazy_static::lazy_static! {
    static ref RUNTIME_DRIVER: RuntimeAdapterStore = RuntimeAdapterStore::new();
}

pub struct QSPIStorage {
    driver: Arc<RefCell<Box<dyn FlashDriver + 'static>>>,
}

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

    fn read_range(&self, global_offset: usize, dest: &mut [u8]) -> Result<(), StorageError> {
        read_range_with_driver(&self.driver, global_offset, dest)
    }
}

pub fn install_runtime_storage_adapter<M, QSPI1>(
    qspi1: QSPI1,
    id1: Identification,
    sys_clk: stm32l4xx_hal::time::Hertz,
) -> Result<StorageMetaHandle, QspiError>
where
    QSPI1: IQspi + 'static,
    M: rtic_monotonics::Monotonic<Duration = config::Duration, Instant = config::Instant> + 'static,
{
    let primary = QSpiDriver::<M>::init(Box::new(qspi1), id1, false, sys_clk)?;
    let geometry = geometry_from_driver(&primary, FlashBanks::One);
    let mapper = BlockMapper::new(geometry);

    let adapter = RuntimeReadAdapter {
        primary,
        secondary: None,
        mapper,
    };
    let meta = StorageMetaHandle::new(geometry);
    meta.set_used_blocks(scan_used_blocks(&adapter));

    install_meta_handle(meta);
    RUNTIME_DRIVER.replace(adapter);
    install_read_range_fn(runtime_read_range);

    Ok(meta)
}

pub fn install_runtime_storage_adapter_dual<M, QSPI1, QSPI2>(
    qspi1: QSPI1,
    id1: Identification,
    qspi2: QSPI2,
    id2: Identification,
    sys_clk: stm32l4xx_hal::time::Hertz,
) -> Result<StorageMetaHandle, QspiError>
where
    QSPI1: IQspi + 'static,
    QSPI2: IQspi + 'static,
    M: rtic_monotonics::Monotonic<Duration = config::Duration, Instant = config::Instant> + 'static,
{
    let primary = QSpiDriver::<M>::init(Box::new(qspi1), id1, false, sys_clk)?;
    let secondary = QSpiDriver::<M>::init(Box::new(qspi2), id2, false, sys_clk)?;
    let geometry = geometry_from_driver(&primary, FlashBanks::Two);
    let mapper = BlockMapper::new(geometry);

    let adapter = RuntimeReadAdapter {
        primary,
        secondary: Some(secondary),
        mapper,
    };
    let meta = StorageMetaHandle::new(geometry);
    meta.set_used_blocks(scan_used_blocks(&adapter));

    install_meta_handle(meta);
    RUNTIME_DRIVER.replace(adapter);
    install_read_range_fn(runtime_read_range);

    Ok(meta)
}

fn geometry_from_driver(driver: &RuntimeDriver, banks: FlashBanks) -> StorageGeometry {
    let guard = driver.borrow();
    let blocks_per_bank = (guard.get_capacity() / config::STORAGE_BLOCK_SIZE_BYTES as usize) as u32;
    let write_granularity_bytes = guard.config().write_max_bytes as u32;

    StorageGeometry {
        block_size_bytes: config::STORAGE_BLOCK_SIZE_BYTES,
        write_granularity_bytes,
        blocks_per_bank,
        banks,
    }
}

fn read_range_with_driver(
    driver: &RuntimeDriver,
    global_offset: usize,
    dest: &mut [u8],
) -> Result<(), StorageError> {
    let mut guard = driver.borrow_mut();

    if let Err(e) = guard.wake_up() {
        return Err(map_qspi_error(e));
    }

    let mut absolute = global_offset;
    let mut written = 0usize;
    while written < dest.len() {
        let (extender, addr) = guard.config().wrap_adress(absolute);
        let to_segment_end = 0x0100_0000usize - addr as usize;
        let chunk = core::cmp::min(dest.len() - written, to_segment_end);

        if let Err(e) = guard.set_addr_extender(extender) {
            return Err(map_qspi_error(e));
        }
        if let Err(e) = guard.read_direct(addr, &mut dest[written..written + chunk]) {
            return Err(map_qspi_error(e));
        }

        written += chunk;
        absolute += chunk;
    }

    guard.want_sleep();
    Ok(())
}

fn read_range_via_adapter(
    adapter: &RuntimeReadAdapter,
    global_offset: usize,
    dest: &mut [u8],
) -> Result<(), StorageError> {
    let block_size = adapter.mapper.geometry().block_size_bytes as usize;
    let mut absolute = global_offset;
    let mut written = 0usize;

    while written < dest.len() {
        let offset_address = adapter
            .mapper
            .map_offset(absolute)
            .map_err(|_| StorageError::InvalidAddress)?;
        let in_block_offset = absolute % block_size;
        let chunk = core::cmp::min(dest.len() - written, block_size - in_block_offset);

        let target_driver = if offset_address.bank_index == 0 {
            &adapter.primary
        } else {
            adapter
                .secondary
                .as_ref()
                .ok_or(StorageError::InvalidAddress)?
        };

        read_range_with_driver(
            target_driver,
            offset_address.local_byte_offset,
            &mut dest[written..written + chunk],
        )?;

        written += chunk;
        absolute += chunk;
    }

    Ok(())
}

fn scan_used_blocks(adapter: &RuntimeReadAdapter) -> u32 {
    let block_size = adapter.mapper.geometry().block_size_bytes as usize;
    let total_blocks = adapter.mapper.total_blocks();
    let mut header = [0xFFu8; 8];

    for global_block in 0..total_blocks {
        let offset = global_block as usize * block_size;
        header.fill(0xFF);

        if read_range_via_adapter(adapter, offset, &mut header).is_err() {
            return global_block;
        }

        let this_block_id = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let prev_block_id = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);

        if core::cmp::min(this_block_id, prev_block_id) == u32::MAX {
            return global_block;
        }
    }

    total_blocks
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

fn map_qspi_error(e: QspiError) -> StorageError {
    match e {
        QspiError::Busy => StorageError::Busy,
        QspiError::Address => StorageError::InvalidAddress,
        QspiError::Unknown => StorageError::Internal,
    }
}

fn runtime_read_range(global_offset: usize, dest: &mut [u8]) -> Result<(), StorageError> {
    let adapter: Option<RuntimeReadAdapter> = RUNTIME_DRIVER.get_cloned();
    if let Some(adapter) = adapter {
        read_range_via_adapter(&adapter, global_offset, dest)
    } else {
        Err(StorageError::NotReady)
    }
}

pub fn runtime_get_mapper() -> Option<BlockMapper> {
    RUNTIME_DRIVER
        .get_cloned()
        .map(|adapter| adapter.mapper.clone())
}
