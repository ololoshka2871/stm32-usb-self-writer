pub mod flash_config;
mod identification;
pub mod qspi_driver;

use core::cell::RefCell;

use identification::Identification;
use qspi_stm32lx3::iqspi::IQspi;

use alloc::{boxed::Box, sync::Arc};

use qspi_driver::{FlashDriver, QSpiDriver};

use qspi_driver::QspiError;

use crate::config;
use crate::main_data_storage::{
    BlockMapper, FlashBanks, StorageContext, StorageError, StorageGeometry, StorageMode,
};

type RuntimeDriver = Arc<RefCell<Box<dyn FlashDriver + 'static>>>;

#[derive(Clone)]
struct RuntimeReadAdapter {
    primary: RuntimeDriver,
    secondary: Option<RuntimeDriver>,
    mapper: BlockMapper,
}

pub struct QSPIStorage {
    adapter: RuntimeReadAdapter,
    geometry: StorageGeometry,
    startup_used_blocks: u32,
}

impl QSPIStorage {
    pub fn new_single<QSPI, M>(
        qspi: QSPI,
        id: Identification,
        sys_clk: stm32l4xx_hal::time::Hertz,
    ) -> Result<Self, QspiError>
    where
        QSPI: IQspi + 'static,
        M: rtic_monotonics::Monotonic<Duration = config::Duration, Instant = config::Instant>
            + 'static,
    {
        let primary = QSpiDriver::<M>::init(Box::new(qspi), id, false, sys_clk)?;
        let geometry = geometry_from_driver(&primary, FlashBanks::One);
        let mapper = BlockMapper::new(geometry);

        let adapter = RuntimeReadAdapter {
            primary,
            secondary: None,
            mapper,
        };

        let startup_used_blocks = scan_used_blocks(&adapter);

        Ok(Self {
            adapter,
            geometry,
            startup_used_blocks,
        })
    }

    pub fn new_dual<QSPI1, QSPI2, M>(
        qspi1: QSPI1,
        id1: Identification,
        qspi2: QSPI2,
        id2: Identification,
        sys_clk: stm32l4xx_hal::time::Hertz,
    ) -> Result<Self, QspiError>
    where
        QSPI1: IQspi + 'static,
        QSPI2: IQspi + 'static,
        M: rtic_monotonics::Monotonic<Duration = config::Duration, Instant = config::Instant>
            + 'static,
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

        let startup_used_blocks = scan_used_blocks(&adapter);

        Ok(Self {
            adapter,
            geometry,
            startup_used_blocks,
        })
    }

    pub fn into_context(self, mode: StorageMode) -> StorageContext {
        let adapter_ptr = Box::into_raw(Box::new(self.adapter)) as usize;

        StorageContext::new(
            mode,
            self.geometry,
            self.startup_used_blocks,
            adapter_ptr,
            read_range_from_context,
            write_block_from_context,
            drop_context_reader,
            erase_from_context,
        )
    }
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

fn read_range_from_context(
    reader_ctx: usize,
    global_offset: usize,
    dest: &mut [u8],
) -> Result<(), StorageError> {
    if reader_ctx == 0 {
        return Err(StorageError::NotReady);
    }

    let adapter = unsafe { &*(reader_ctx as *const RuntimeReadAdapter) };
    read_range_via_adapter(adapter, global_offset, dest)
}

fn erase_with_driver(driver: &RuntimeDriver) -> Result<(), StorageError> {
    let mut guard = driver.borrow_mut();

    guard.wake_up().map_err(map_qspi_error)?;
    guard.erase().map_err(map_qspi_error)?;
    guard.want_sleep();

    Ok(())
}

fn write_block_with_driver(
    driver: &RuntimeDriver,
    local_offset: usize,
    data: &[u8],
) -> Result<(), StorageError> {
    let mut guard = driver.borrow_mut();

    guard.wake_up().map_err(map_qspi_error)?;

    let (extender, addr) = guard.config().wrap_adress(local_offset);
    guard.set_addr_extender(extender).map_err(map_qspi_error)?;
    guard.write_block(addr, data).map_err(map_qspi_error)?;
    guard.want_sleep();

    Ok(())
}

fn write_block_via_adapter(
    adapter: &RuntimeReadAdapter,
    global_block_index: u32,
    data: &[u8],
) -> Result<(), StorageError> {
    let block = adapter
        .mapper
        .map_block(global_block_index)
        .map_err(|_| StorageError::InvalidAddress)?;

    let target_driver = if block.bank_index == 0 {
        &adapter.primary
    } else {
        adapter
            .secondary
            .as_ref()
            .ok_or(StorageError::InvalidAddress)?
    };

    let local_offset = block.local_block_index as usize * adapter.mapper.geometry().block_size_bytes as usize;
    write_block_with_driver(target_driver, local_offset, data)
}

fn write_block_from_context(
    reader_ctx: usize,
    global_block_index: u32,
    data: &[u8],
) -> Result<(), StorageError> {
    if reader_ctx == 0 {
        return Err(StorageError::NotReady);
    }

    let adapter = unsafe { &*(reader_ctx as *const RuntimeReadAdapter) };
    write_block_via_adapter(adapter, global_block_index, data)
}

fn erase_all_via_adapter(adapter: &RuntimeReadAdapter) -> Result<(), StorageError> {
    erase_with_driver(&adapter.primary)?;

    if let Some(secondary) = adapter.secondary.as_ref() {
        erase_with_driver(secondary)?;
    }

    Ok(())
}

fn erase_from_context(reader_ctx: usize) -> Result<(), StorageError> {
    if reader_ctx == 0 {
        return Err(StorageError::NotReady);
    }

    let adapter = unsafe { &*(reader_ctx as *const RuntimeReadAdapter) };
    erase_all_via_adapter(adapter)
}

fn drop_context_reader(reader_ctx: usize) {
    if reader_ctx == 0 {
        return;
    }

    unsafe {
        drop(Box::from_raw(reader_ctx as *mut RuntimeReadAdapter));
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
