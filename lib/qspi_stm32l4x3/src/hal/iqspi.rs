use crate::qspi::{QspiConfig, QspiError, QspiReadCommand, QspiWriteCommand};

pub trait IQspi {
    fn fmode(&self) -> u8;
    fn is_busy(&self) -> bool;
    fn abort_transmission(&self);

    fn get_config(&self) -> QspiConfig;
    fn apply_config(&mut self, config: QspiConfig);

    fn transfer(&self, command: QspiReadCommand, buffer: &mut [u8]) -> Result<(), QspiError>;
    fn write(&self, command: QspiWriteCommand) -> Result<(), QspiError>;

    fn start_memory_mapping(&self, command: QspiWriteCommand) -> Result<(), QspiError>;
}
