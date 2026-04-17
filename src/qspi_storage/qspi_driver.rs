use core::cell::RefCell;

use alloc::{boxed::Box, sync::Arc};
use qspi_stm32lx3::{QspiConfig, iqspi::IQspi, qspi::QspiWriteCommand};

#[cfg(feature = "stm32l433")]
pub use qspi_stm32lx3::{qspi, stm32l4x3::QUADSPI};

pub use qspi::{
    ClkPin, IO0Pin, IO1Pin, IO2Pin, IO3Pin, NCSPin, Qspi, QspiError, QspiMode, QspiReadCommand,
};
use rtic_monotonics::Monotonic;

use crate::config;

use super::flash_config::FlashConfig;

// https://github.com/jonas-schievink/spi-memory/blob/master/src/series25.rs

#[allow(unused)]
#[repr(u8)]
pub enum Opcode {
    /// Read the 8-bit legacy device ID.
    ReadDeviceId = 0xAB,
    /// Read the 8-bit manufacturer and device IDs.
    ReadMfDId = 0x90,
    /// Read 16-bit manufacturer ID and 8-bit device ID.
    ReadJedecId = 0x9F,
    /// Set the write enable latch.
    WriteEnable = 0x06,
    /// Clear the write enable latch.
    WriteDisable = 0x04,
    /// Read the 8-bit status register.
    ReadStatus = 0x05,
    /// Write the 8-bit status register. Not all bits are writeable.
    WriteStatus = 0x01,
    Read = 0x03,
    PageProg = 0x02, // directly writes to EEPROMs too
    SectorErase = 0x20,
    BlockErase = 0xD8,
    ChipErase = 0xC7,
    /// Read 16-bit manufacturer ID and 8-bit device ID. MultiIO
    ReadJedecIdMIO = 0xAF,
    /// Read flags register
    ReadFlagStatus = 0x70,
    /// QIO fast read 3 byte address
    QIOFastRead = 0xEB,
    /// QIO fast write 3 byte address 1-256 bytes
    QIOFastProgramm = 0x32,
    /// Write address extander register
    WriteAddrExtanderReg = 0xC5,
}
#[allow(dead_code)]
pub trait FlashDriver: Sync + Send {
    fn get_capacity(&self) -> usize;
    fn erase(&mut self) -> Result<(), QspiError>;

    /// Run this before any write operation
    fn write_enable(&mut self) -> Result<(), QspiError>;
    fn raw_read(&mut self, command: QspiReadCommand, buffer: &mut [u8]) -> Result<(), QspiError>;
    fn raw_write(&mut self, command: QspiWriteCommand) -> Result<(), QspiError>;
    fn read_direct(&mut self, start_address: u32, dest: &mut [u8]) -> Result<(), QspiError>;
    fn write_block(&mut self, start_address: u32, data: &[u8]) -> Result<(), QspiError>;
    fn config(&self) -> &FlashConfig;
    fn apply_qspi_config(&mut self, cfg: QspiConfig);
    fn set_memory_mapping_mode(&mut self, enable: bool) -> Result<(), QspiError>;
    fn set_addr_extender(&mut self, extender_value: u32) -> Result<(), QspiError>;
    fn wake_up(&mut self) -> Result<(), QspiError>;
    fn want_sleep(&mut self);
}

#[derive(PartialEq)]
enum SleepState {
    Slepping,
    Waiting { sleep_at: config::Instant },
    Working,
}

pub struct QSpiDriver<M> {
    qspi: Box<dyn IQspi>,
    config: &'static FlashConfig,
    extender_value: u32,
    //sleep_timer: Timer,
    sleep_state: SleepState,
    dual: bool,
    _m: core::marker::PhantomData<M>,
}

impl<M: Monotonic<Duration = config::Duration, Instant = config::Instant> + 'static> QSpiDriver<M> {
    pub fn init(
        mut qspi: Box<dyn IQspi>,
        id: super::Identification,
        dual: bool,
        sys_clk: stm32l4xx_hal::time::Hertz,
    ) -> Result<Arc<RefCell<Box<dyn FlashDriver>>>, QspiError> {
        let config = QspiConfig::default()
            /* failsafe config */
            .clock_prescaler((sys_clk.0 / 1_000_000) as u8)
            .clock_mode(qspi::ClockMode::Mode3);

        qspi.apply_config(config);

        let config = super::flash_config::FLASH_CONFIGS.iter().find_map(|cfg| {
            if cfg.vendor_id == id.mfr_code() && cfg.capacity_code == id.device_id()[1] {
                Some(cfg)
            } else {
                None
            }
        });

        if let Some(config) = config {
            defmt::info!("Found flash: {}", config);
            let mut res = Self {
                qspi,
                config,
                extender_value: 0xff,
                sleep_state: SleepState::Slepping,
                dual,
                _m: core::marker::PhantomData,
            };

            res.wake_up()?;

            if let Err(e) = config.configure(&mut res, dual, sys_clk) {
                defmt::error!("Failed to init flash!");
                Err(e)
            } else {
                defmt::info!("Initialised QSPI flash: {}", config);

                let b: Box<dyn FlashDriver> = Box::new(res);
                Ok(Arc::new(RefCell::new(b)))
            }
        } else {
            defmt::error!("Unknown QSPI flash JDEC ID: {}", defmt::Debug2Format(&id));
            Err(QspiError::Unknown)
        }
    }

    fn is_memory_mapped(&self) -> bool {
        self.qspi.is_memory_mapped()
    }

    fn enter_sleep(&mut self) -> Result<(), QspiError> {
        self.cancel_memory_mapping()?;

        let wake_up_cmd = QspiWriteCommand {
            instruction: Some((
                self.config.enter_deep_sleep_command_code,
                QspiMode::SingleChannel,
            )),
            address: None,
            alternative_bytes: None,
            dummy_cycles: 0, // internal register, no wait
            data: None,
            double_data_rate: false,
        };

        self.qspi.write(wake_up_cmd)?;

        self.sleep_state = SleepState::Slepping;

        defmt::trace!("Flash sleep...");

        Ok(())
    }

    fn is_busy(&mut self, qspi_mode: bool) -> Result<bool, QspiError> {
        self.cancel_memory_mapping()?;
        (self.config.is_busy)(self, qspi_mode)
    }

    fn cancel_memory_mapping(&mut self) -> Result<(), QspiError> {
        self.set_memory_mapping_mode(false)
    }
}

impl<M: Monotonic<Duration = config::Duration, Instant = config::Instant> + 'static> FlashDriver
    for QSpiDriver<M>
{
    fn get_capacity(&self) -> usize {
        self.config.capacity(self.dual)
    }

    fn erase(&mut self) -> Result<(), QspiError> {
        self.cancel_memory_mapping()?;

        self.wake_up()?;

        if (self.config.is_busy)(self, true)? {
            self.want_sleep();
            Err(QspiError::Busy)
        } else {
            (self.config.chip_erase)(self, true)?;
            self.want_sleep();
            Ok(())
        }
    }

    fn write_enable(&mut self) -> Result<(), QspiError> {
        let write_enable_command = QspiWriteCommand {
            instruction: Some((Opcode::WriteEnable as u8, QspiMode::SingleChannel)),
            address: None,
            alternative_bytes: None,
            dummy_cycles: 0,
            data: None,
            double_data_rate: false,
        };
        self.qspi.write(write_enable_command)
    }

    fn raw_read(&mut self, command: QspiReadCommand, buffer: &mut [u8]) -> Result<(), QspiError> {
        self.qspi.transfer(command, buffer)
    }

    fn raw_write(&mut self, command: QspiWriteCommand) -> Result<(), QspiError> {
        self.qspi.write(command)
    }

    fn read_direct(&mut self, start_address: u32, dest: &mut [u8]) -> Result<(), QspiError> {
        self.cancel_memory_mapping()?;

        let read_command = QspiReadCommand {
            instruction: Some((Opcode::QIOFastRead as u8, QspiMode::QuadChannel)),
            address: Some((start_address, QspiMode::QuadChannel)),
            alternative_bytes: None,
            dummy_cycles: self.config.read_dumy_cycles,
            double_data_rate: false,
            data_mode: QspiMode::QuadChannel,
            receive_length: dest.len() as u32,
        };

        self.raw_read(read_command, dest)
    }

    fn write_block(&mut self, start_address: u32, data: &[u8]) -> Result<(), QspiError> {
        self.cancel_memory_mapping()?;

        for start in (0..data.len()).step_by(self.config.write_max_bytes) {
            let end = if data.len() - start >= self.config.write_max_bytes {
                start + self.config.write_max_bytes
            } else {
                data.len()
            };
            let block = &data[start..end];

            while self.is_busy(true)? {
                /* wait write complead */
                //freertos_rust::CurrentTask::delay(Duration::ticks(1));
            }

            // Sets the write enable latch bit before each PROGRAM, ERASE, and WRITE command.
            self.write_enable()?;

            (self.config.check_write_ok)(self, true)?;

            let write_cmd = QspiWriteCommand {
                instruction: Some((Opcode::QIOFastProgramm as u8, QspiMode::QuadChannel)),
                address: Some((start_address + start as u32, QspiMode::QuadChannel)),
                alternative_bytes: None,
                dummy_cycles: self.config.write_dumy_cycles,
                data: Some((block, QspiMode::QuadChannel)),
                double_data_rate: false,
            };

            self.qspi.write(write_cmd)?;
        }

        Ok(())
    }

    fn config(&self) -> &FlashConfig {
        self.config
    }

    fn apply_qspi_config(&mut self, cfg: QspiConfig) {
        self.qspi.apply_config(cfg)
    }

    fn set_memory_mapping_mode(&mut self, enable: bool) -> Result<(), QspiError> {
        if enable {
            if enable == self.is_memory_mapped() {
                return Ok(());
            }

            const DUMMY: [u8; 1] = [0];
            let enable_mapping_cmd = QspiWriteCommand {
                instruction: Some((Opcode::QIOFastRead as u8, QspiMode::QuadChannel)),
                address: Some((0, QspiMode::QuadChannel)),
                alternative_bytes: None,
                dummy_cycles: self.config.read_dumy_cycles,
                data: Some((&DUMMY, QspiMode::QuadChannel)),
                double_data_rate: false,
            };

            self.qspi.start_memory_mapping(enable_mapping_cmd)
        } else {
            self.qspi.abort_transmission();
            Ok(())
        }
    }

    fn set_addr_extender(&mut self, extender_value: u32) -> Result<(), QspiError> {
        if self.extender_value != extender_value {
            self.cancel_memory_mapping()?;

            //self.write_enable()?;

            let ext_size = self.config.address_size;
            let data = extender_value.to_le_bytes();
            let set_extender_cmd = QspiWriteCommand {
                instruction: Some((Opcode::WriteAddrExtanderReg as u8, QspiMode::QuadChannel)),
                address: None,
                alternative_bytes: None,
                dummy_cycles: 0, // internal register, no wait
                data: Some((&data[..ext_size.bytes()], QspiMode::QuadChannel)),
                double_data_rate: false,
            };

            self.qspi.write(set_extender_cmd)?;

            self.extender_value = extender_value;

            defmt::debug!("New map flash segment: {}", extender_value);
        }
        Ok(())
    }

    fn wake_up(&mut self) -> Result<(), QspiError> {
        #[cfg(feature = "flash-sleep")]
        {
            if self.sleep_state == SleepState::Slepping {
                self.cancel_memory_mapping()?;

                let wake_up_cmd = QspiWriteCommand {
                    instruction: Some((self.config.wake_up_command_code, QspiMode::SingleChannel)),
                    address: None,
                    alternative_bytes: None,
                    dummy_cycles: 0, // internal register, no wait
                    data: None,
                    double_data_rate: false,
                };

                self.qspi.write(wake_up_cmd)?;

                defmt::trace!("Flash wake up...");
            }
        }

        self.sleep_state = SleepState::Working;
        Ok(())
    }

    fn want_sleep(&mut self) {
        #[cfg(feature = "flash-sleep")]
        {
            if self.sleep_state == SleepState::Working {
                self.sleep_state = SleepState::Waiting {
                    sleep_at: M::now() + config::Duration::secs(1),
                };
            }
        }
    }
}

// Маркерные трейты, чтобы наконец позволить сделать таймер, захватывающий драйвер в лямбду
unsafe impl<M> Sync for QSpiDriver<M> {}

unsafe impl<M> Send for QSpiDriver<M> {}

//-----------------------------------------------------------------------------

pub fn get_jedec_id_cfg(
    qspi: &mut dyn qspi_stm32lx3::iqspi::IQspi,
    use_qspi: bool,
) -> Result<super::Identification, QspiError> {
    let get_id_command = QspiReadCommand {
        instruction: if use_qspi {
            Some((Opcode::ReadJedecIdMIO as u8, QspiMode::QuadChannel))
        } else {
            Some((Opcode::ReadJedecId as u8, QspiMode::SingleChannel))
        },
        address: None,
        alternative_bytes: None,
        dummy_cycles: 0,
        data_mode: if use_qspi {
            QspiMode::QuadChannel
        } else {
            QspiMode::SingleChannel
        },
        receive_length: 3,
        double_data_rate: false,
    };
    let mut id_arr = [0; 3];

    qspi.transfer(get_id_command, &mut id_arr)?;

    if id_arr == [0, 0, 0] || id_arr == [0xff, 0xff, 0xff] {
        Err(QspiError::Unknown)
    } else {
        Ok(super::Identification::from_jedec_id(&id_arr))
    }
}
