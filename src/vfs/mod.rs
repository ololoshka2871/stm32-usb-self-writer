mod callbacks;
mod static_data;

use core::{cell::RefCell, usize};

use alloc::{boxed::Box, sync::Arc, vec::Vec};

use emfat_rust::{EntryBuilder, emfat_entry, emfat_t};

use heatshrink_rust::CompressedData;
use my_proc_macro::c_str;
use usbd_scsi::{BlockDevice, BlockDeviceError};

pub type SettingsAccessor = dyn Fn() -> crate::settings::AppSettings;

pub struct EMfatStorage {
    ctx: emfat_t,
    _fstable: Vec<emfat_entry>, // обязательно должно быть heap-allocated, так как заимствуется в emfat по указателю
    settings_accessor: Arc<RefCell<Option<Box<SettingsAccessor>>>>,
    storage_context: Box<crate::main_data_storage::StorageContext>,
}

struct StaticBinData {
    data: &'static [u8],
}

// terminate strings with '\0' c_str("text") for strlen() compatible

impl EMfatStorage {
    pub fn new(disk_label: &str, storage_context: crate::main_data_storage::StorageContext) -> Self {
        let settings_accessor = Arc::new(RefCell::new(None));
        let storage_context = Box::new(storage_context);
        let mut ctx = unsafe { core::mem::MaybeUninit::zeroed().assume_init() };
        let storage_context_ptr = (&*storage_context) as *const crate::main_data_storage::StorageContext
            as usize;
        let mut fstable = Self::build_files_table(settings_accessor.clone(), storage_context_ptr);

        emfat_rust::emfat_rust_init(&mut ctx, disk_label, fstable.as_mut_ptr());
        Self {
            ctx,
            _fstable: fstable,
            settings_accessor,
            storage_context,
        }
    }

    fn build_files_table(
        settings_accessor: Arc<RefCell<Option<Box<SettingsAccessor>>>>,
        storage_context_ptr: usize,
    ) -> Vec<emfat_entry> {
        #[allow(unused_imports)]
        use callbacks::{flash_read, meminfo_read, settings_read, unpack_reader};
        use static_data::{DRIVER_INF_COMPRESSED, PROTO_COMPRESSED, README_COMPRESSED};

        defmt::trace!("EmFat: Registring virtual files:");

        let mut res: Vec<emfat_entry> = Vec::new();

        defmt::trace!("EmFat: /");
        res.push(EntryBuilder::new().name(c_str!("")).dir(true).build());

        defmt::trace!("EmFat: /Readme.txt");
        res.push(
            EntryBuilder::new()
                .name(c_str!("Readme.txt"))
                .lvl(1)
                .size(README_COMPRESSED.original_size)
                .max_size(README_COMPRESSED.original_size)
                .read_cb(Some(unpack_reader))
                .user_data(&README_COMPRESSED as *const CompressedData as usize)
                .build(),
        );
        defmt::trace!("EmFat: /driver.inf");
        res.push(
            EntryBuilder::new()
                .name(c_str!("driver.inf"))
                .lvl(1)
                .size(DRIVER_INF_COMPRESSED.original_size)
                .max_size(DRIVER_INF_COMPRESSED.original_size)
                .read_cb(Some(unpack_reader))
                .user_data(&DRIVER_INF_COMPRESSED as *const CompressedData as usize)
                .build(),
        );

        defmt::trace!("EmFat: /proto.prt");
        res.push(
            EntryBuilder::new()
                .name(c_str!("proto.prt"))
                .lvl(1)
                .size(PROTO_COMPRESSED.original_size)
                .max_size(PROTO_COMPRESSED.original_size)
                .read_cb(Some(unpack_reader))
                .user_data(&PROTO_COMPRESSED as *const CompressedData as usize)
                .build(),
        );

        defmt::trace!("EmFat: /settings.var");
        res.push(
            EntryBuilder::new()
                .name(c_str!("config.var"))
                .lvl(1)
                .size(2048) // noauto, размер может меняться - это генерированный текст
                .max_size(2048)
                .read_cb(Some(settings_read))
                .user_data({
                    let boxed = Box::leak(Box::new(settings_accessor));
                    boxed as *const Arc<RefCell<Option<Box<SettingsAccessor>>>> as usize
                })
                .build(),
        );

        let raw_view = Box::leak(Box::new(callbacks::FlashReadUserData {
            storage_context: storage_context_ptr as *const crate::main_data_storage::StorageContext,
            used_view: false,
        })) as *const callbacks::FlashReadUserData
            as usize;

        let used_view = Box::leak(Box::new(callbacks::FlashReadUserData {
            storage_context: storage_context_ptr as *const crate::main_data_storage::StorageContext,
            used_view: true,
        })) as *const callbacks::FlashReadUserData
            as usize;

        let storage_context =
            unsafe { &*(storage_context_ptr as *const crate::main_data_storage::StorageContext) };

        defmt::trace!("EmFat: /storage.var");
        res.push(
            EntryBuilder::new()
                .name(c_str!("storage.var"))
                .lvl(1)
                .size(512)
                .max_size(2048)
                .read_cb(Some(meminfo_read))
                .user_data(storage_context_ptr)
                .build(),
        );

        {
            let flash_size = storage_context.raw_size_bytes();
            defmt::trace!("EmFat: /data_raw.hs ({} B)", flash_size);
            res.push(
                EntryBuilder::new()
                    .name(c_str!("data_raw.hs"))
                    .lvl(1)
                    .size(flash_size)
                    .max_size(flash_size)
                    .read_cb(Some(flash_read))
                    .user_data(raw_view)
                    .build(),
            );
        }

        {
            let used = storage_context.used_size_bytes();
            defmt::trace!("EmFat: /data_use.hs ({})", used);
            res.push(
                EntryBuilder::new()
                    .name(c_str!("data_use.hs"))
                    .lvl(1)
                    .size(used)
                    .max_size(used)
                    .read_cb(Some(flash_read))
                    .user_data(used_view)
                    .build(),
            );
        }

        res.push(EntryBuilder::terminator_entry());

        res
    }

    pub fn set_settings_accessor(&mut self, accessor: Box<SettingsAccessor>) {
        self.settings_accessor.borrow_mut().replace(accessor);
    }

    pub fn process_pending_erase(&mut self) -> Result<bool, crate::main_data_storage::StorageError> {
        self.storage_context.process_pending_erase()
    }
}


impl BlockDevice for EMfatStorage {
    const BLOCK_BYTES: usize = 512;

    fn read_block(&mut self, lba: u32, block: &mut [u8]) -> Result<(), BlockDeviceError> {
        if self.storage_context.is_erase_in_progress() {
            defmt::warn!("Read blocked: storage erase is in progress");
            return Err(BlockDeviceError::NotReady);
        }

        defmt::trace!("SCSI: Get LBA {:#x}", lba);
        unsafe {
            emfat_rust::emfat_read(&mut self.ctx, block.as_mut_ptr(), lba, 1);
        }
        Ok(())
    }

    fn write_block(&mut self, _lba: u32, _block: &[u8]) -> Result<(), BlockDeviceError> {
        //defmt::trace!("SCSI: Write LBA block {}", lba);
        //unsafe { emfat_rust::emfat_write(&mut self.ctx, block.as_ptr(), lba, 1) }
        //Ok(())
        Err(BlockDeviceError::HardwareError)
    }

    fn max_lba(&self) -> u32 {
        //defmt::trace!("SCSI: Get max LBA {}", self.ctx.disk_sectors);
        self.ctx.disk_sectors // Это не размер а максимальный номер блока по 512 байт
    }

    fn is_write_protected(&self) -> bool {
        true
    }

    fn is_ready(&self) -> bool {
        !self.storage_context.is_erase_in_progress()
    }
}

unsafe impl Send for EMfatStorage {}
