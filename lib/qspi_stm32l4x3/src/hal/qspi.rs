//! Quad Serial Peripheral Interface (QSPI) bus for L4x3
use stm32l4xx_hal as hal;

use super::iqspi::IQspi;

// Пины для L4x3 для QSPI
#[cfg(feature = "stm32l443")]
use hal::gpio::gpiob::PB10;
use hal::gpio::{
    gpioa::{PA2, PA3, PA6, PA7},
    gpiob::{PB0, PB1, PB11},
    gpiod::{PD3, PD4, PD5, PD6, PD7},
    gpioe::{PE10, PE11, PE12, PE13, PE14, PE15},
};

use crate::stm32l4x3::QUADSPI;
use crate::{
    hal::rcc::{Enable, AHB3},
    QspiConfig,
};

use core::{
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    ptr,
};
use cortex_m::interrupt::Mutex;

use hal::gpio::{Alternate, PushPull, Speed};

#[doc(hidden)]
mod private {
    pub trait Sealed {}
}

/// CLK pin. This trait is sealed and cannot be implemented.
pub trait ClkPin<QSPI>: private::Sealed {
    fn set_speed(self, speed: Speed) -> Self;
}
/// nCS pin. This trait is sealed and cannot be implemented.
pub trait NCSPin<QSPI>: private::Sealed {
    fn set_speed(self, speed: Speed) -> Self;
}
/// IO0 pin. This trait is sealed and cannot be implemented.
pub trait IO0Pin<QSPI>: private::Sealed {
    fn set_speed(self, speed: Speed) -> Self;
}
/// IO1 pin. This trait is sealed and cannot be implemented.
pub trait IO1Pin<QSPI>: private::Sealed {
    fn set_speed(self, speed: Speed) -> Self;
}
/// IO2 pin. This trait is sealed and cannot be implemented.
pub trait IO2Pin<QSPI>: private::Sealed {
    fn set_speed(self, speed: Speed) -> Self;
}
/// IO3 pin. This trait is sealed and cannot be implemented.
pub trait IO3Pin<QSPI>: private::Sealed {
    fn set_speed(self, speed: Speed) -> Self;
}

pub trait IntoVirtualClk: private::Sealed + Sized {
    type RealClk;

    fn virtual_clk(&self) -> VirtualClk<Self::RealClk>;
}

pub struct VirtualClk<CLK> {
    _marker: PhantomData<CLK>,
}

impl<CLK> VirtualClk<CLK> {
    fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<CLK> private::Sealed for VirtualClk<CLK> {}

impl<CLK> ClkPin<QUADSPI> for VirtualClk<CLK>
where
    CLK: ClkPin<QUADSPI>,
{
    fn set_speed(self, _speed: Speed) -> Self {
        self
    }
}

impl<CLK> IntoVirtualClk for CLK
where
    CLK: ClkPin<QUADSPI>,
{
    type RealClk = CLK;

    fn virtual_clk(&self) -> VirtualClk<Self::RealClk> {
        VirtualClk::new()
    }
}

pub trait QspiRegisterAccess {
    fn with_qspi<R>(&self, action: impl FnOnce(&QUADSPI) -> R) -> R;
    fn with_qspi_mut<R>(&self, action: impl FnOnce(&mut QUADSPI) -> R) -> R;
}

pub struct SharedQUADSPI {
    lock_flag: Mutex<Cell<bool>>,
    qspi: UnsafeCell<QUADSPI>,
}

pub struct SharedQUADSPILock<'a> {
    shared: &'a SharedQUADSPI,
}

impl SharedQUADSPI {
    pub fn new(qspi: QUADSPI, ahb3: &mut AHB3) -> Self {
        QUADSPI::enable(ahb3);

        qspi.cr.modify(|_, w| w.en().clear_bit());
        qspi.fcr.write(|w| {
            w.ctof()
                .set_bit()
                .csmf()
                .set_bit()
                .ctcf()
                .set_bit()
                .ctef()
                .set_bit()
        });

        Self {
            lock_flag: Mutex::new(Cell::new(false)),
            qspi: UnsafeCell::new(qspi),
        }
    }

    fn try_acquire(&self) -> bool {
        cortex_m::interrupt::free(|cs| {
            let flag = self.lock_flag.borrow(cs);
            if flag.get() {
                false
            } else {
                flag.set(true);
                true
            }
        })
    }

    fn release(&self) {
        cortex_m::interrupt::free(|cs| {
            self.lock_flag.borrow(cs).set(false);
        });
    }

    pub fn lock(&self) -> SharedQUADSPILock<'_> {
        while !self.try_acquire() {
            cortex_m::asm::nop();
        }
        SharedQUADSPILock { shared: self }
    }
}

unsafe impl Sync for SharedQUADSPI {}

impl Drop for SharedQUADSPILock<'_> {
    fn drop(&mut self) {
        self.shared.release();
    }
}

impl QspiRegisterAccess for SharedQUADSPI {
    fn with_qspi<R>(&self, action: impl FnOnce(&QUADSPI) -> R) -> R {
        let guard = self.lock();
        guard.with_qspi(action)
    }

    fn with_qspi_mut<R>(&self, action: impl FnOnce(&mut QUADSPI) -> R) -> R {
        let guard = self.lock();
        guard.with_qspi_mut(action)
    }
}

impl<'a> QspiRegisterAccess for SharedQUADSPILock<'a> {
    fn with_qspi<R>(&self, action: impl FnOnce(&QUADSPI) -> R) -> R {
        unsafe { action(&*self.shared.qspi.get()) }
    }

    fn with_qspi_mut<R>(&self, action: impl FnOnce(&mut QUADSPI) -> R) -> R {
        unsafe { action(&mut *self.shared.qspi.get()) }
    }
}

/// &SharedQUADSPI is also a valid access token — allows two channels
/// to borrow the same SharedQUADSPI simultaneously.
impl<'a> QspiRegisterAccess for &'a SharedQUADSPI {
    fn with_qspi<R>(&self, action: impl FnOnce(&QUADSPI) -> R) -> R {
        (*self).with_qspi(action)
    }

    fn with_qspi_mut<R>(&self, action: impl FnOnce(&mut QUADSPI) -> R) -> R {
        (*self).with_qspi_mut(action)
    }
}

macro_rules! pins {
    ($qspi:ident, $af:literal, CLK: [$($clk:ident),*], nCS: [$($ncs:ident),*],
        IO0: [$($io0:ident),*], IO1: [$($io1:ident),*], IO2: [$($io2:ident),*],
        IO3: [$($io3:ident),*]) => {
        $(
            impl private::Sealed for $clk<Alternate<PushPull, $af>> {}
            impl ClkPin<$qspi> for $clk<Alternate<PushPull, $af>> {
                fn set_speed(self, speed: Speed) -> Self{
                    self.set_speed(speed)
                }
            }
        )*
        $(
            impl private::Sealed for $ncs<Alternate<PushPull, $af>> {}
            impl NCSPin<$qspi> for $ncs<Alternate<PushPull, $af>> {
                fn set_speed(self, speed: Speed) -> Self{
                    self.set_speed(speed)
                }
            }
        )*
        $(
            impl private::Sealed for $io0<Alternate<PushPull, $af>> {}
            impl IO0Pin<$qspi> for $io0<Alternate<PushPull, $af>> {
                fn set_speed(self, speed: Speed) -> Self{
                    self.set_speed(speed)
                }
            }
        )*
        $(
            impl private::Sealed for $io1<Alternate<PushPull, $af>> {}
            impl IO1Pin<$qspi> for $io1<Alternate<PushPull, $af>> {
                fn set_speed(self, speed: Speed) -> Self{
                    self.set_speed(speed)
                }
            }
        )*
        $(
            impl private::Sealed for $io2<Alternate<PushPull, $af>> {}
            impl IO2Pin<$qspi> for $io2<Alternate<PushPull, $af>> {
                fn set_speed(self, speed: Speed) -> Self{
                    self.set_speed(speed)
                }
            }
        )*
        $(
            impl private::Sealed for $io3<Alternate<PushPull, $af>> {}
            impl IO3Pin<$qspi> for $io3<Alternate<PushPull, $af>> {
                fn set_speed(self, speed: Speed) -> Self{
                    self.set_speed(speed)
                }
            }
        )*
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum QspiMode {
    SingleChannel = 0b01,
    DualChannel = 0b10,
    QuadChannel = 0b11,
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum AddressSize {
    Addr8Bit = 0b00,
    Addr16Bit = 0b01,
    Addr24Bit = 0b10,
    Addr32Bit = 0b11,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SampleShift {
    None,
    HalfACycle,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ClockMode {
    Mode0,
    Mode3,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum QspiError {
    Busy,
    Address,
    Unknown,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FlashBank {
    Bank1,
    Bank2,
    Both,
}

impl FlashBank {
    pub fn to_bank_bit(&self) -> bool {
        match self {
            FlashBank::Bank1 => false,
            _ => true,
        }
    }

    pub fn is_dual(&self) -> bool {
        matches!(self, FlashBank::Both)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct QspiWriteCommand<'a> {
    pub instruction: Option<(u8, QspiMode)>,
    pub address: Option<(u32, QspiMode)>,
    pub alternative_bytes: Option<(&'a [u8], QspiMode)>,
    pub dummy_cycles: u8,
    pub data: Option<(&'a [u8], QspiMode)>,
    pub double_data_rate: bool,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct QspiReadCommand<'a> {
    pub instruction: Option<(u8, QspiMode)>,
    pub address: Option<(u32, QspiMode)>,
    pub alternative_bytes: Option<(&'a [u8], QspiMode)>,
    /// check Command Set
    pub dummy_cycles: u8,
    pub data_mode: QspiMode,
    pub receive_length: u32,
    pub double_data_rate: bool,
}

impl<'a> QspiWriteCommand<'a> {
    pub fn address(self, addr: u32, mode: QspiMode) -> Self {
        QspiWriteCommand {
            address: Some((addr, mode)),
            ..self
        }
    }

    pub fn alternative_bytes(self, bytes: &'a [u8], mode: QspiMode) -> Self {
        QspiWriteCommand {
            alternative_bytes: Some((bytes, mode)),
            ..self
        }
    }

    pub fn dummy_cycles(self, n: u8) -> Self {
        QspiWriteCommand {
            dummy_cycles: n,
            ..self
        }
    }

    pub fn data(self, bytes: &'a [u8], mode: QspiMode) -> Self {
        QspiWriteCommand {
            data: Some((bytes, mode)),
            ..self
        }
    }
}

impl<'a> QspiReadCommand<'a> {
    pub fn address(self, addr: u32, mode: QspiMode) -> Self {
        QspiReadCommand {
            address: Some((addr, mode)),
            ..self
        }
    }

    pub fn alternative_bytes(self, bytes: &'a [u8], mode: QspiMode) -> Self {
        QspiReadCommand {
            alternative_bytes: Some((bytes, mode)),
            ..self
        }
    }

    pub fn dummy_cycles(self, n: u8) -> Self {
        QspiReadCommand {
            dummy_cycles: n,
            ..self
        }
    }

    pub fn receive_length(self, length: u32) -> Self {
        QspiReadCommand {
            receive_length: length,
            ..self
        }
    }
}

pub struct Qspi<PINS> {
    qspi: QUADSPI,
    pins: PINS,
    config: QspiConfig,
    flash_bank: FlashBank,
}

impl<CLK, NCS, IO0, IO1, IO2, IO3> Qspi<(CLK, NCS, IO0, IO1, IO2, IO3)> {
    pub fn new_bank1(
        qspi: QUADSPI,
        pins: (CLK, NCS, IO0, IO1, IO2, IO3),
        ahb3: &mut AHB3,
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
        Self::init(qspi, pins, ahb3, config, FlashBank::Bank1)
    }

    pub fn new_bank2(
        qspi: QUADSPI,
        pins: (CLK, NCS, IO0, IO1, IO2, IO3),
        ahb3: &mut AHB3,
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
        Self::init(qspi, pins, ahb3, config, FlashBank::Bank2)
    }

    pub fn new(
        qspi: QUADSPI,
        pins: (CLK, NCS, IO0, IO1, IO2, IO3),
        ahb3: &mut AHB3,
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
        Self::new_bank1(qspi, pins, ahb3, config)
    }
}

impl<CLK, NCS1, IO0_1, IO1_1, IO2_1, IO3_1, NCS2, IO0_2, IO1_2, IO2_2, IO3_2>
    Qspi<(
        CLK,
        NCS1,
        IO0_1,
        IO1_1,
        IO2_1,
        IO3_1,
        NCS2,
        IO0_2,
        IO1_2,
        IO2_2,
        IO3_2,
    )>
{
    pub fn new_dual(
        qspi: QUADSPI,
        pins: (
            CLK,
            NCS1,
            IO0_1,
            IO1_1,
            IO2_1,
            IO3_1,
            NCS2,
            IO0_2,
            IO1_2,
            IO2_2,
            IO3_2,
        ),
        ahb3: &mut AHB3,
        config: QspiConfig,
    ) -> Self
    where
        CLK: ClkPin<QUADSPI>,
        NCS1: NCSPin<QUADSPI>,
        IO0_1: IO0Pin<QUADSPI>,
        IO1_1: IO1Pin<QUADSPI>,
        IO2_1: IO2Pin<QUADSPI>,
        IO3_1: IO3Pin<QUADSPI>,
        NCS2: NCSPin<QUADSPI>,
        IO0_2: IO0Pin<QUADSPI>,
        IO1_2: IO1Pin<QUADSPI>,
        IO2_2: IO2Pin<QUADSPI>,
        IO3_2: IO3Pin<QUADSPI>,
    {
        // Enable quad SPI in the clocks.
        QUADSPI::enable(ahb3);

        // Disable QUADSPI before configuring it.
        qspi.cr.modify(|_, w| w.en().clear_bit());

        // Clear all pending flags.
        qspi.fcr.write(|w| {
            w.ctof()
                .set_bit()
                .csmf()
                .set_bit()
                .ctcf()
                .set_bit()
                .ctef()
                .set_bit()
        });

        let mut unit = Qspi {
            qspi,
            pins: pins.set_very_high_speed(),
            config,
            flash_bank: FlashBank::Both,
        };
        unit.apply_config(config);
        unit
    }
}

impl<PINS> Qspi<PINS> {
    fn init(
        qspi: QUADSPI,
        pins: PINS,
        ahb3: &mut AHB3,
        config: QspiConfig,
        flash_bank: FlashBank,
    ) -> Self
    where
        PINS: QspiPins,
    {
        // Enable quad SPI in the clocks.
        QUADSPI::enable(ahb3);

        // Disable QUADSPI before configuring it.
        qspi.cr.modify(|_, w| w.en().clear_bit());

        // Clear all pending flags.
        qspi.fcr.write(|w| {
            w.ctof()
                .set_bit()
                .csmf()
                .set_bit()
                .ctcf()
                .set_bit()
                .ctef()
                .set_bit()
        });

        let mut unit = Qspi {
            qspi,
            pins: pins.set_very_high_speed(),
            config,
            flash_bank,
        };
        unit.apply_config(config);
        unit
    }
}

impl<PINS> super::iqspi::IQspi for Qspi<PINS> {
    fn fmode(&self) -> u8 {
        self.qspi.ccr.read().fmode().bits()
    }

    fn is_busy(&self) -> bool {
        self.qspi.sr.read().busy().bit_is_set()
    }

    /// Aborts any ongoing transaction
    /// Note can cause problems if aborting writes to flash satus register
    fn abort_transmission(&self) {
        self.qspi.cr.modify(|_, w| w.abort().set_bit());
        while self.qspi.sr.read().busy().bit_is_set() {}
    }

    fn get_config(&self) -> QspiConfig {
        self.config
    }

    fn apply_config(&mut self, config: QspiConfig) {
        if self.qspi.sr.read().busy().bit_is_set() {
            self.abort_transmission();
        }

        self.qspi
            .cr
            .modify(|_, w| unsafe { w.fthres().bits(config.fifo_threshold as u8) });

        while self.qspi.sr.read().busy().bit_is_set() {}

        // Modify the prescaler and select flash bank / dual flash mode.
        self.qspi.cr.modify(|_, w| unsafe {
            w.prescaler()
                .bits(config.clock_prescaler as u8)
                .fsel()
                .bit(self.flash_bank.to_bank_bit())
                .dfm()
                .bit(self.flash_bank.is_dual())
                .sshift()
                .bit(config.sample_shift == SampleShift::HalfACycle)
        });
        while self.is_busy() {}

        // Modify DCR with flash size, CSHT and clock mode
        self.qspi.dcr.modify(|_, w| unsafe {
            w.fsize()
                .bits(config.flash_size as u8)
                .csht()
                .bits(config.chip_select_high_time as u8)
                .ckmode()
                .bit(config.clock_mode == ClockMode::Mode3)
        });
        while self.is_busy() {}

        // Enable QSPI
        self.qspi.cr.modify(|_, w| w.en().set_bit());
        while self.is_busy() {}

        self.config = config;
    }

    fn transfer(&self, command: QspiReadCommand, buffer: &mut [u8]) -> Result<(), QspiError> {
        if self.is_busy() {
            return Err(QspiError::Busy);
        }

        // If double data rate change shift
        if command.double_data_rate {
            self.qspi.cr.modify(|_, w| w.sshift().bit(false));
        }
        while self.is_busy() {}

        // Clear the transfer complete flag.
        self.qspi.fcr.modify(|_, w| w.ctcf().set_bit());

        let mut dmode: u8 = 0;
        let mut instruction: u8 = 0;
        let mut imode: u8 = 0;
        let mut admode: u8 = 0;
        let mut adsize: u8 = 0;
        let mut abmode: u8 = 0;
        let mut absize: u8 = 0;

        // Write the length and format of data
        if command.receive_length > 0 {
            self.qspi
                .dlr
                .write(|w| unsafe { w.dl().bits(command.receive_length as u32 - 1) });
            if self.config.qpi_mode {
                dmode = QspiMode::QuadChannel as u8;
            } else {
                dmode = command.data_mode as u8;
            }
        }

        // Write instruction mode
        if let Some((inst, mode)) = command.instruction {
            if self.config.qpi_mode {
                imode = QspiMode::QuadChannel as u8;
            } else {
                imode = mode as u8;
            }
            instruction = inst;
        }

        // Note Address mode
        if let Some((_, mode)) = command.address {
            if self.config.qpi_mode {
                admode = QspiMode::QuadChannel as u8;
            } else {
                admode = mode as u8;
            }
            adsize = self.config.address_size as u8;
        }

        // Write Alternative bytes
        if let Some((a_bytes, mode)) = command.alternative_bytes {
            if self.config.qpi_mode {
                abmode = QspiMode::QuadChannel as u8;
            } else {
                abmode = mode as u8;
            }

            absize = a_bytes.len() as u8 - 1;

            self.qspi.abr.write(|w| {
                let mut reg_byte: u32 = 0;
                for (i, element) in a_bytes.iter().rev().enumerate() {
                    reg_byte |= (*element as u32) << (i * 8);
                }
                unsafe { w.alternate().bits(reg_byte) }
            });
        }

        // Write CCR register with instruction etc.
        self.qspi.ccr.modify(|_, w| unsafe {
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

        // Write address, triggers send
        if let Some((addr, _)) = command.address {
            self.qspi.ar.write(|w| unsafe { w.address().bits(addr) });

            // Transfer error
            if self.qspi.sr.read().tef().bit_is_set() {
                return Err(QspiError::Address);
            }
        }

        // Transfer error
        if self.qspi.sr.read().tef().bit_is_set() {
            return Err(QspiError::Unknown);
        }

        // Read data from the buffer
        let mut b = buffer.iter_mut();
        while self.qspi.sr.read().tcf().bit_is_clear() {
            if self.qspi.sr.read().ftf().bit_is_set() {
                if let Some(v) = b.next() {
                    unsafe {
                        *v = ptr::read_volatile(&self.qspi.dr as *const _ as *const u8);
                    }
                } else {
                    // OVERFLOW
                }
            }
        }
        // When transfer complete, empty fifo buffer
        while self.qspi.sr.read().flevel().bits() > 0 {
            if let Some(v) = b.next() {
                unsafe {
                    *v = ptr::read_volatile(&self.qspi.dr as *const _ as *const u8);
                }
            } else {
                // OVERFLOW
            }
        }
        // If double data rate set shift back to original and if busy abort.
        if command.double_data_rate {
            if self.is_busy() {
                self.abort_transmission();
            }
            self.qspi.cr.modify(|_, w| {
                w.sshift()
                    .bit(self.config.sample_shift == SampleShift::HalfACycle)
            });
        }
        while self.is_busy() {}
        self.qspi.fcr.write(|w| w.ctcf().set_bit());
        Ok(())
    }

    fn write(&self, command: QspiWriteCommand) -> Result<(), QspiError> {
        if self.is_busy() {
            return Err(QspiError::Busy);
        }
        // Clear the transfer complete flag.
        self.qspi.fcr.modify(|_, w| w.ctcf().set_bit());

        let mut dmode: u8 = 0;
        let mut instruction: u8 = 0;
        let mut imode: u8 = 0;
        let mut admode: u8 = 0;
        let mut adsize: u8 = 0;
        let mut abmode: u8 = 0;
        let mut absize: u8 = 0;

        // Write the length and format of data
        if let Some((data, mode)) = command.data {
            self.qspi
                .dlr
                .write(|w| unsafe { w.dl().bits(data.len() as u32 - 1) });
            if self.config.qpi_mode {
                dmode = QspiMode::QuadChannel as u8;
            } else {
                dmode = mode as u8;
            }
        }

        // Write instruction mode
        if let Some((inst, mode)) = command.instruction {
            if self.config.qpi_mode {
                imode = QspiMode::QuadChannel as u8;
            } else {
                imode = mode as u8;
            }
            instruction = inst;
        }

        // Note Address mode
        if let Some((_, mode)) = command.address {
            if self.config.qpi_mode {
                admode = QspiMode::QuadChannel as u8;
            } else {
                admode = mode as u8;
            }
            adsize = self.config.address_size as u8;
        }

        // Write Alternative bytes
        if let Some((a_bytes, mode)) = command.alternative_bytes {
            if self.config.qpi_mode {
                abmode = QspiMode::QuadChannel as u8;
            } else {
                abmode = mode as u8;
            }

            absize = a_bytes.len() as u8 - 1;

            self.qspi.abr.write(|w| {
                let mut reg_byte: u32 = 0;
                for (i, element) in a_bytes.iter().rev().enumerate() {
                    reg_byte |= (*element as u32) << (i * 8);
                }
                unsafe { w.alternate().bits(reg_byte) }
            });
        }

        if command.double_data_rate {
            self.qspi.cr.modify(|_, w| w.sshift().bit(false));
        }

        // Write CCR register with instruction etc.
        self.qspi.ccr.modify(|_, w| unsafe {
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

        // Write address, triggers send
        if let Some((addr, _)) = command.address {
            self.qspi.ar.write(|w| unsafe { w.address().bits(addr) });
        }

        // Transfer error
        if self.qspi.sr.read().tef().bit_is_set() {
            return Err(QspiError::Unknown);
        }

        // Write data to the FIFO
        if let Some((data, _)) = command.data {
            for byte in data {
                while self.qspi.sr.read().ftf().bit_is_clear() {}
                unsafe {
                    #[allow(invalid_reference_casting)]
                    ptr::write_volatile(&self.qspi.dr as *const _ as *mut u8, *byte);
                }
            }
        }

        while self.qspi.sr.read().tcf().bit_is_clear() {}

        self.qspi.fcr.write(|w| w.ctcf().set_bit());

        if self.is_busy() {}

        if command.double_data_rate {
            self.qspi.cr.modify(|_, w| {
                w.sshift()
                    .bit(self.config.sample_shift == SampleShift::HalfACycle)
            });
        }
        Ok(())
    }

    fn start_memory_mapping(&self, command: QspiWriteCommand) -> Result<(), QspiError> {
        if self.is_busy() {
            return Err(QspiError::Busy);
        }

        // stop module
        self.qspi.cr.modify(|_, w| w.en().clear_bit());

        // Clear the transfer complete flag.
        self.qspi.fcr.modify(|_, w| w.ctcf().set_bit());

        let mut dmode: u8 = 0;
        let mut instruction: u8 = 0;
        let mut imode: u8 = 0;
        let mut admode: u8 = 0;
        let mut adsize: u8 = 0;
        let mut abmode: u8 = 0;
        let mut absize: u8 = 0;

        // data size - max
        self.qspi.dlr.write(|w| unsafe { w.dl().bits(u32::MAX) });

        // Write the length and format of data
        if let Some((_, mode)) = command.data {
            /*
            self.qspi
                .dlr
                .write(|w| unsafe { w.dl().bits(data.len() as u32 - 1) });
            */
            if self.config.qpi_mode {
                dmode = QspiMode::QuadChannel as u8;
            } else {
                dmode = mode as u8;
            }
        }

        // Write instruction mode
        if let Some((inst, mode)) = command.instruction {
            if self.config.qpi_mode {
                imode = QspiMode::QuadChannel as u8;
            } else {
                imode = mode as u8;
            }
            instruction = inst;
        }

        // Note Address mode
        if let Some((_, mode)) = command.address {
            if self.config.qpi_mode {
                admode = QspiMode::QuadChannel as u8;
            } else {
                admode = mode as u8;
            }
            adsize = self.config.address_size as u8;
        }

        // Write Alternative bytes
        if let Some((a_bytes, mode)) = command.alternative_bytes {
            if self.config.qpi_mode {
                abmode = QspiMode::QuadChannel as u8;
            } else {
                abmode = mode as u8;
            }

            absize = a_bytes.len() as u8 - 1;

            self.qspi.abr.write(|w| {
                let mut reg_byte: u32 = 0;
                for (i, element) in a_bytes.iter().rev().enumerate() {
                    reg_byte |= (*element as u32) << (i * 8);
                }
                unsafe { w.alternate().bits(reg_byte) }
            });
        }

        if command.double_data_rate {
            self.qspi.cr.modify(|_, w| w.sshift().bit(false));
        }

        // Write CCR register with instruction etc.
        self.qspi.ccr.modify(|_, w| unsafe {
            w.sioo()
                .clear_bit() // no sioo
                .fmode()
                .bits(0b11) // memory mapped mode
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

        /*
        // in QSPI mode address from address bus, so ignore this
        // Write address, triggers send
        if let Some((addr, _)) = command.address {
            self.qspi.ar.write(|w| unsafe { w.address().bits(addr) });
        }
        */

        /*
        // Write data to the FIFO
        if let Some((data, _)) = command.data {
            for byte in data {
                while self.qspi.sr.read().ftf().bit_is_clear() {}
                unsafe {
                    ptr::write_volatile(&self.qspi.dr as *const _ as *mut u8, *byte);
                }
            }
        }
        */

        //while self.qspi.sr.read().tcf().bit_is_clear() {}

        //self.qspi.fcr.write(|w| w.ctcf().set_bit());

        //if self.is_busy() {}

        if command.double_data_rate {
            self.qspi.cr.modify(|_, w| {
                w.sshift()
                    .bit(self.config.sample_shift == SampleShift::HalfACycle)
            });
        }

        // enable module
        self.qspi.cr.modify(|_, w| w.en().set_bit());

        /*
        // Transfer error
        if self.qspi.sr.read().tef().bit_is_set() {
            return Err(QspiError::Unknown);
        }
        */

        Ok(())
    }
}

impl<CLK, NCS, IO0, IO1, IO2, IO3> Qspi<(CLK, NCS, IO0, IO1, IO2, IO3)> {
    pub fn destroy(self) -> (QUADSPI, (CLK, NCS, IO0, IO1, IO2, IO3)) {
        (self.qspi, self.pins)
    }
}

impl<CLK, NCS1, IO0_1, IO1_1, IO2_1, IO3_1, NCS2, IO0_2, IO1_2, IO2_2, IO3_2>
    Qspi<(
        CLK,
        NCS1,
        IO0_1,
        IO1_1,
        IO2_1,
        IO3_1,
        NCS2,
        IO0_2,
        IO1_2,
        IO2_2,
        IO3_2,
    )>
{
    pub fn destroy(
        self,
    ) -> (
        QUADSPI,
        (
            CLK,
            NCS1,
            IO0_1,
            IO1_1,
            IO2_1,
            IO3_1,
            NCS2,
            IO0_2,
            IO1_2,
            IO2_2,
            IO3_2,
        ),
    ) {
        (self.qspi, self.pins)
    }
}

pub trait QspiPins {
    fn set_very_high_speed(self) -> Self;
}

impl<CLK, NCS, IO0, IO1, IO2, IO3> QspiPins for (CLK, NCS, IO0, IO1, IO2, IO3)
where
    CLK: ClkPin<QUADSPI>,
    NCS: NCSPin<QUADSPI>,
    IO0: IO0Pin<QUADSPI>,
    IO1: IO1Pin<QUADSPI>,
    IO2: IO2Pin<QUADSPI>,
    IO3: IO3Pin<QUADSPI>,
{
    fn set_very_high_speed(self) -> Self {
        (
            self.0.set_speed(Speed::VeryHigh),
            self.1.set_speed(Speed::VeryHigh),
            self.2.set_speed(Speed::VeryHigh),
            self.3.set_speed(Speed::VeryHigh),
            self.4.set_speed(Speed::VeryHigh),
            self.5.set_speed(Speed::VeryHigh),
        )
    }
}

impl<CLK, NCS1, IO0_1, IO1_1, IO2_1, IO3_1, NCS2, IO0_2, IO1_2, IO2_2, IO3_2> QspiPins
    for (
        CLK,
        NCS1,
        IO0_1,
        IO1_1,
        IO2_1,
        IO3_1,
        NCS2,
        IO0_2,
        IO1_2,
        IO2_2,
        IO3_2,
    )
where
    CLK: ClkPin<QUADSPI>,
    NCS1: NCSPin<QUADSPI>,
    IO0_1: IO0Pin<QUADSPI>,
    IO1_1: IO1Pin<QUADSPI>,
    IO2_1: IO2Pin<QUADSPI>,
    IO3_1: IO3Pin<QUADSPI>,
    NCS2: NCSPin<QUADSPI>,
    IO0_2: IO0Pin<QUADSPI>,
    IO1_2: IO1Pin<QUADSPI>,
    IO2_2: IO2Pin<QUADSPI>,
    IO3_2: IO3Pin<QUADSPI>,
{
    fn set_very_high_speed(self) -> Self {
        (
            self.0.set_speed(Speed::VeryHigh),
            self.1.set_speed(Speed::VeryHigh),
            self.2.set_speed(Speed::VeryHigh),
            self.3.set_speed(Speed::VeryHigh),
            self.4.set_speed(Speed::VeryHigh),
            self.5.set_speed(Speed::VeryHigh),
            self.6.set_speed(Speed::VeryHigh),
            self.7.set_speed(Speed::VeryHigh),
            self.8.set_speed(Speed::VeryHigh),
            self.9.set_speed(Speed::VeryHigh),
            self.10.set_speed(Speed::VeryHigh),
        )
    }
}

#[cfg(feature = "stm32l433")]
pins!(
    QUADSPI,
    10,
    CLK: [PA3, PE10],
    nCS: [PA2, PB11, PE11, PD3],
    IO0: [PB1, PE12, PD4],
    IO1: [PB0, PE13, PD5],
    IO2: [PA7, PE14, PD6],
    IO3: [PA6, PE15, PD7]
);

#[cfg(feature = "stm32l443")]
pins!(
    QUADSPI,
    10,
    CLK: [PB10, PE10],
    nCS: [PA2, PB11, PE11],
    IO0: [PB1, PE12],
    IO1: [PB0, PE13],
    IO2: [PA7, PE14],
    IO3: [PA6, PE15]
);
