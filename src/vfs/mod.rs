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
}

struct StaticBinData {
    data: &'static [u8],
}

// terminate strings with '\0' c_str("text") for strlen() compatible

impl EMfatStorage {
    pub fn new(disk_label: &str) -> Self {
        let settings_accessor = Arc::new(RefCell::new(None));
        let mut ctx = unsafe { core::mem::MaybeUninit::zeroed().assume_init() };
        let mut fstable = Self::build_files_table(settings_accessor.clone());

        emfat_rust::emfat_rust_init(&mut ctx, disk_label, fstable.as_mut_ptr());
        Self {
            ctx,
            _fstable: fstable,
            settings_accessor,
        }
    }

    fn build_files_table(
        settings_accessor: Arc<RefCell<Option<Box<SettingsAccessor>>>>,
    ) -> Vec<emfat_entry> {
        #[allow(unused_imports)]
        use callbacks::{/*flash_read, meminfo_read,*/ settings_read, unpack_reader};
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

        //#[cfg(not(feature = "no-flash"))]
        //{
        //    defmt::trace!("EmFat: /storage.var");
        //    res.push(
        //        EntryBuilder::new()
        //            .name(c_str!("storage.var"))
        //            .lvl(1)
        //            .size(512) // noauto, размер может меняться - это генерированный текст
        //            .max_size(2048)
        //            .read_cb(Some(meminfo_read))
        //            .build(),
        //    );
        //
        //    {
        //        let flash_size = crate::main_data_storage::flash_size();
        //        defmt::trace!("EmFat: /data_raw.hs ({} B)", flash_size);
        //        res.push(
        //            EntryBuilder::new()
        //                .name(c_str!("data_raw.hs"))
        //                .lvl(1)
        //                .size(flash_size)
        //                .max_size(flash_size)
        //                .read_cb(Some(flash_read))
        //                .build(),
        //        );
        //    }
        //
        //    match crate::main_data_storage::memory_state() {
        //        crate::main_data_storage::MemoryState::Undefined => {
        //            defmt::error!("EmFat: /data_use.hs <undefined state>")
        //        }
        //        crate::main_data_storage::MemoryState::PartialUsed(pages) => {
        //            if pages == 0 {
        //                defmt::debug!("EmFat: /data_use.hs <empty-skipped>");
        //            } else {
        //                let used = (pages * crate::main_data_storage::flash_page_size()) as usize;
        //                defmt::trace!("EmFat: /data_use.hs ({})", used);
        //                res.push(
        //                    EntryBuilder::new()
        //                        .name(c_str!("data_use.hs"))
        //                        .lvl(1)
        //                        .size(used)
        //                        .max_size(used)
        //                        .read_cb(Some(flash_read))
        //                        .build(),
        //                );
        //            }
        //        }
        //        crate::main_data_storage::MemoryState::FullUsed => {
        //            defmt::debug!("EmFat: /data_use.hs <full used>")
        //        }
        //    }
        //
        //    res.push(EntryBuilder::terminator_entry());
        //}

        res
    }

    pub fn set_settings_accessor(&mut self, accessor: Box<SettingsAccessor>) {
        self.settings_accessor.borrow_mut().replace(accessor);
    }
}

impl BlockDevice for EMfatStorage {
    const BLOCK_BYTES: usize = 512;

    fn read_block(&mut self, lba: u32, block: &mut [u8]) -> Result<(), BlockDeviceError> {
        // TODO:
        //let res = if crate::main_data_storage::is_erase_in_progress() {
        //    defmt::warn!("Read error: flash is busy");
        //    Err(BlockDeviceError::NotReady)
        //} else {
        //    //defmt::debug!("SCSI: Read LBA block {}", lba);
        //    unsafe {
        //        emfat_rust::emfat_read(&mut self.ctx, block.as_mut_ptr(), lba, 1);
        //    }
        //    Ok(())
        //};
        //
        //res

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
        //!crate::main_data_storage::is_erase_in_progress()
        true
    }
}

unsafe impl Send for EMfatStorage {}
