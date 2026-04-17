use core::ptr;

use crate::{
    hal::QspiConfig,
    iqspi::IQspi,
    qspi::{
        ClkPin, ClockMode, FlashBank, IO0Pin, IO1Pin, IO2Pin, IO3Pin, NCSPin, QspiError, QspiMode,
        QspiPins, QspiReadCommand, QspiRegisterAccess, QspiWriteCommand, SampleShift,
    },
    stm32l4x3::QUADSPI,
};

pub struct QspiSharedChannel<ACCESS, PINS> {
    access: ACCESS,
    pins: PINS,
    config: QspiConfig,
    flash_bank: FlashBank,
}

impl<ACCESS, CLK, NCS, IO0, IO1, IO2, IO3> QspiSharedChannel<ACCESS, (CLK, NCS, IO0, IO1, IO2, IO3)>
where
    ACCESS: QspiRegisterAccess,
{
    pub fn new_bank1(
        access: ACCESS,
        pins: (CLK, NCS, IO0, IO1, IO2, IO3),
        config: QspiConfig,
    ) -> Self
    where
        CLK: ClkPin<QUADSPI>,
        NCS: NCSPin<QUADSPI>,
        IO0: IO0Pin<QUADSPI>,
        IO1: IO1Pin<QUADSPI>,
        IO2: IO2Pin<QUADSPI>,
        IO3: IO3Pin<QUADSPI>,
    {
        Self::init(access, pins, config, FlashBank::Bank1)
    }

    pub fn new_bank2(
        access: ACCESS,
        pins: (CLK, NCS, IO0, IO1, IO2, IO3),
        config: QspiConfig,
    ) -> Self
    where
        CLK: ClkPin<QUADSPI>,
        NCS: NCSPin<QUADSPI>,
        IO0: IO0Pin<QUADSPI>,
        IO1: IO1Pin<QUADSPI>,
        IO2: IO2Pin<QUADSPI>,
        IO3: IO3Pin<QUADSPI>,
    {
        Self::init(access, pins, config, FlashBank::Bank2)
    }

    pub fn new(access: ACCESS, pins: (CLK, NCS, IO0, IO1, IO2, IO3), config: QspiConfig) -> Self
    where
        CLK: ClkPin<QUADSPI>,
        NCS: NCSPin<QUADSPI>,
        IO0: IO0Pin<QUADSPI>,
        IO1: IO1Pin<QUADSPI>,
        IO2: IO2Pin<QUADSPI>,
        IO3: IO3Pin<QUADSPI>,
    {
        Self::new_bank1(access, pins, config)
    }

    pub fn destroy(self) -> (ACCESS, (CLK, NCS, IO0, IO1, IO2, IO3)) {
        (self.access, self.pins)
    }
}

impl<ACCESS, PINS> QspiSharedChannel<ACCESS, PINS>
where
    ACCESS: QspiRegisterAccess,
{
    fn init(access: ACCESS, pins: PINS, config: QspiConfig, flash_bank: FlashBank) -> Self
    where
        PINS: QspiPins,
    {
        let mut unit = Self {
            access,
            pins: pins.set_very_high_speed(),
            config,
            flash_bank,
        };
        unit.apply_config(config);
        unit
    }

    fn is_busy_raw(qspi: &QUADSPI) -> bool {
        qspi.sr.read().busy().bit_is_set()
    }

    fn abort_raw(qspi: &QUADSPI) {
        qspi.cr.modify(|_, w| w.abort().set_bit());
        while qspi.sr.read().busy().bit_is_set() {}
    }
}

impl<ACCESS, PINS> IQspi for QspiSharedChannel<ACCESS, PINS>
where
    ACCESS: QspiRegisterAccess,
{
    fn bank(&self) -> FlashBank {
        self.flash_bank
    }

    fn is_memory_mapped(&self) -> bool {
        self.access.with_qspi(|qspi| {
            let cr = qspi.cr.read();

            (cr.fsel().bit() == self.flash_bank.to_bank_bit())
                && (cr.dfm().bit() == self.flash_bank.is_dual())
                && (qspi.ccr.read().fmode().bits() == 0b11)
        })
    }

    fn is_busy(&self) -> bool {
        self.access
            .with_qspi(|qspi| qspi.sr.read().busy().bit_is_set())
    }

    fn abort_transmission(&self) {
        self.access.with_qspi(|qspi| Self::abort_raw(qspi));
    }

    fn get_config(&self) -> QspiConfig {
        self.config
    }

    fn apply_config(&mut self, config: QspiConfig) {
        self.config = config;
        self.access.with_qspi(|qspi| {
            if Self::is_busy_raw(qspi) {
                Self::abort_raw(qspi);
            }

            qspi.cr
                .modify(|_, w| unsafe { w.fthres().bits(config.fifo_threshold as u8) });

            while Self::is_busy_raw(qspi) {}

            qspi.cr.modify(|_, w| unsafe {
                w.prescaler()
                    .bits(config.clock_prescaler as u8)
                    .fsel()
                    .bit(self.flash_bank.to_bank_bit())
                    .dfm()
                    .bit(self.flash_bank.is_dual())
                    .sshift()
                    .bit(config.sample_shift == SampleShift::HalfACycle)
            });
            while Self::is_busy_raw(qspi) {}

            qspi.dcr.modify(|_, w| unsafe {
                w.fsize()
                    .bits(config.flash_size as u8)
                    .csht()
                    .bits(config.chip_select_high_time as u8)
                    .ckmode()
                    .bit(config.clock_mode == ClockMode::Mode3)
            });
            while Self::is_busy_raw(qspi) {}

            qspi.cr.modify(|_, w| w.en().set_bit());
            while Self::is_busy_raw(qspi) {}
        });
    }

    fn transfer(&self, command: QspiReadCommand, buffer: &mut [u8]) -> Result<(), QspiError> {
        self.access.with_qspi(|qspi| {
            qspi.cr.modify(|_, w| {
                w.fsel()
                    .bit(self.flash_bank.to_bank_bit())
                    .dfm()
                    .bit(self.flash_bank.is_dual())
            });

            if Self::is_busy_raw(qspi) {
                return Err(QspiError::Busy);
            }

            if command.double_data_rate {
                qspi.cr.modify(|_, w| w.sshift().bit(false));
            }
            while Self::is_busy_raw(qspi) {}

            qspi.fcr.modify(|_, w| w.ctcf().set_bit());

            let mut dmode: u8 = 0;
            let mut instruction: u8 = 0;
            let mut imode: u8 = 0;
            let mut admode: u8 = 0;
            let mut adsize: u8 = 0;
            let mut abmode: u8 = 0;
            let mut absize: u8 = 0;

            if command.receive_length > 0 {
                qspi.dlr
                    .write(|w| unsafe { w.dl().bits(command.receive_length as u32 - 1) });
                if self.config.qpi_mode {
                    dmode = QspiMode::QuadChannel as u8;
                } else {
                    dmode = command.data_mode as u8;
                }
            }

            if let Some((inst, mode)) = command.instruction {
                if self.config.qpi_mode {
                    imode = QspiMode::QuadChannel as u8;
                } else {
                    imode = mode as u8;
                }
                instruction = inst;
            }

            if let Some((_, mode)) = command.address {
                if self.config.qpi_mode {
                    admode = QspiMode::QuadChannel as u8;
                } else {
                    admode = mode as u8;
                }
                adsize = self.config.address_size as u8;
            }

            if let Some((a_bytes, mode)) = command.alternative_bytes {
                if self.config.qpi_mode {
                    abmode = QspiMode::QuadChannel as u8;
                } else {
                    abmode = mode as u8;
                }

                absize = a_bytes.len() as u8 - 1;

                qspi.abr.write(|w| {
                    let mut reg_byte: u32 = 0;
                    for (i, element) in a_bytes.iter().rev().enumerate() {
                        reg_byte |= (*element as u32) << (i * 8);
                    }
                    unsafe { w.alternate().bits(reg_byte) }
                });
            }

            qspi.ccr.modify(|_, w| unsafe {
                w.fmode()
                    .bits(0b01)
                    .admode()
                    .bits(admode)
                    .adsize()
                    .bits(adsize)
                    .abmode()
                    .bits(abmode)
                    .absize()
                    .bits(absize)
                    .ddrm()
                    .bit(command.double_data_rate)
                    .dcyc()
                    .bits(command.dummy_cycles)
                    .dmode()
                    .bits(dmode)
                    .imode()
                    .bits(imode)
                    .instruction()
                    .bits(instruction)
            });

            if let Some((addr, _)) = command.address {
                qspi.ar.write(|w| unsafe { w.address().bits(addr) });
                if qspi.sr.read().tef().bit_is_set() {
                    return Err(QspiError::Address);
                }
            }

            if qspi.sr.read().tef().bit_is_set() {
                return Err(QspiError::Unknown);
            }

            let mut b = buffer.iter_mut();
            while qspi.sr.read().tcf().bit_is_clear() {
                if qspi.sr.read().ftf().bit_is_set() {
                    if let Some(v) = b.next() {
                        unsafe {
                            *v = ptr::read_volatile(&qspi.dr as *const _ as *const u8);
                        }
                    }
                }
            }

            while qspi.sr.read().flevel().bits() > 0 {
                if let Some(v) = b.next() {
                    unsafe {
                        *v = ptr::read_volatile(&qspi.dr as *const _ as *const u8);
                    }
                }
            }

            if command.double_data_rate {
                if Self::is_busy_raw(qspi) {
                    Self::abort_raw(qspi);
                }
                qspi.cr.modify(|_, w| {
                    w.sshift()
                        .bit(self.config.sample_shift == SampleShift::HalfACycle)
                });
            }

            while Self::is_busy_raw(qspi) {}
            qspi.fcr.write(|w| w.ctcf().set_bit());

            Ok(())
        })
    }

    fn write(&self, command: QspiWriteCommand) -> Result<(), QspiError> {
        self.access.with_qspi(|qspi| {
            qspi.cr.modify(|_, w| {
                w.fsel()
                    .bit(self.flash_bank.to_bank_bit())
                    .dfm()
                    .bit(self.flash_bank.is_dual())
            });

            if Self::is_busy_raw(qspi) {
                return Err(QspiError::Busy);
            }

            qspi.fcr.modify(|_, w| w.ctcf().set_bit());

            let mut dmode: u8 = 0;
            let mut instruction: u8 = 0;
            let mut imode: u8 = 0;
            let mut admode: u8 = 0;
            let mut adsize: u8 = 0;
            let mut abmode: u8 = 0;
            let mut absize: u8 = 0;

            if let Some((data, mode)) = command.data {
                qspi.dlr
                    .write(|w| unsafe { w.dl().bits(data.len() as u32 - 1) });
                if self.config.qpi_mode {
                    dmode = QspiMode::QuadChannel as u8;
                } else {
                    dmode = mode as u8;
                }
            }

            if let Some((inst, mode)) = command.instruction {
                if self.config.qpi_mode {
                    imode = QspiMode::QuadChannel as u8;
                } else {
                    imode = mode as u8;
                }
                instruction = inst;
            }

            if let Some((_, mode)) = command.address {
                if self.config.qpi_mode {
                    admode = QspiMode::QuadChannel as u8;
                } else {
                    admode = mode as u8;
                }
                adsize = self.config.address_size as u8;
            }

            if let Some((a_bytes, mode)) = command.alternative_bytes {
                if self.config.qpi_mode {
                    abmode = QspiMode::QuadChannel as u8;
                } else {
                    abmode = mode as u8;
                }

                absize = a_bytes.len() as u8 - 1;

                qspi.abr.write(|w| {
                    let mut reg_byte: u32 = 0;
                    for (i, element) in a_bytes.iter().rev().enumerate() {
                        reg_byte |= (*element as u32) << (i * 8);
                    }
                    unsafe { w.alternate().bits(reg_byte) }
                });
            }

            if command.double_data_rate {
                qspi.cr.modify(|_, w| w.sshift().bit(false));
            }

            qspi.ccr.modify(|_, w| unsafe {
                w.fmode()
                    .bits(0b00)
                    .admode()
                    .bits(admode)
                    .adsize()
                    .bits(adsize)
                    .abmode()
                    .bits(abmode)
                    .absize()
                    .bits(absize)
                    .ddrm()
                    .bit(command.double_data_rate)
                    .dcyc()
                    .bits(command.dummy_cycles)
                    .dmode()
                    .bits(dmode)
                    .imode()
                    .bits(imode)
                    .instruction()
                    .bits(instruction)
            });

            if let Some((addr, _)) = command.address {
                qspi.ar.write(|w| unsafe { w.address().bits(addr) });
            }

            if qspi.sr.read().tef().bit_is_set() {
                return Err(QspiError::Unknown);
            }

            if let Some((data, _)) = command.data {
                for byte in data {
                    while qspi.sr.read().ftf().bit_is_clear() {}
                    unsafe {
                        #[allow(invalid_reference_casting)]
                        ptr::write_volatile(&qspi.dr as *const _ as *mut u8, *byte);
                    }
                }
            }

            while qspi.sr.read().tcf().bit_is_clear() {}

            qspi.fcr.write(|w| w.ctcf().set_bit());

            if command.double_data_rate {
                qspi.cr.modify(|_, w| {
                    w.sshift()
                        .bit(self.config.sample_shift == SampleShift::HalfACycle)
                });
            }

            Ok(())
        })
    }

    fn start_memory_mapping(&self, command: QspiWriteCommand) -> Result<(), QspiError> {
        let bank_info = self.flash_bank;
        self.access.with_qspi(|qspi| {
            //if Self::is_busy_raw(qspi) {
            //    return Err(QspiError::Busy);
            //}

            qspi.cr.modify(|_, w| w.en().clear_bit());
            qspi.ccr.modify(|_, w| unsafe { w.fmode().bits(0b01) });
            qspi.cr.modify(move |_, w| {
                w.fsel()
                    .bit(bank_info.to_bank_bit())
                    .dfm()
                    .bit(bank_info.is_dual())
            });
            qspi.fcr.modify(|_, w| w.ctcf().set_bit());

            let mut dmode: u8 = 0;
            let mut instruction: u8 = 0;
            let mut imode: u8 = 0;
            let mut admode: u8 = 0;
            let mut adsize: u8 = 0;
            let mut abmode: u8 = 0;
            let mut absize: u8 = 0;

            qspi.dlr.write(|w| unsafe { w.dl().bits(u32::MAX) });

            if let Some((_, mode)) = command.data {
                if self.config.qpi_mode {
                    dmode = QspiMode::QuadChannel as u8;
                } else {
                    dmode = mode as u8;
                }
            }

            if let Some((inst, mode)) = command.instruction {
                if self.config.qpi_mode {
                    imode = QspiMode::QuadChannel as u8;
                } else {
                    imode = mode as u8;
                }
                instruction = inst;
            }

            if let Some((_, mode)) = command.address {
                if self.config.qpi_mode {
                    admode = QspiMode::QuadChannel as u8;
                } else {
                    admode = mode as u8;
                }
                adsize = self.config.address_size as u8;
            }

            if let Some((a_bytes, mode)) = command.alternative_bytes {
                if self.config.qpi_mode {
                    abmode = QspiMode::QuadChannel as u8;
                } else {
                    abmode = mode as u8;
                }

                absize = a_bytes.len() as u8 - 1;

                qspi.abr.write(|w| {
                    let mut reg_byte: u32 = 0;
                    for (i, element) in a_bytes.iter().rev().enumerate() {
                        reg_byte |= (*element as u32) << (i * 8);
                    }
                    unsafe { w.alternate().bits(reg_byte) }
                });
            }

            if command.double_data_rate {
                qspi.cr.modify(|_, w| w.sshift().bit(false));
            }

            qspi.ccr.modify(|_, w| unsafe {
                w.sioo()
                    .clear_bit()
                    .fmode()
                    .bits(0b11)
                    .admode()
                    .bits(admode)
                    .adsize()
                    .bits(adsize)
                    .abmode()
                    .bits(abmode)
                    .absize()
                    .bits(absize)
                    .ddrm()
                    .bit(command.double_data_rate)
                    .dcyc()
                    .bits(command.dummy_cycles)
                    .dmode()
                    .bits(dmode)
                    .imode()
                    .bits(imode)
                    .instruction()
                    .bits(instruction)
            });

            if command.double_data_rate {
                qspi.cr.modify(|_, w| {
                    w.sshift()
                        .bit(self.config.sample_shift == SampleShift::HalfACycle)
                });
            }

            qspi.cr.modify(|_, w| w.en().set_bit());
            Ok(())
        })
    }
}
