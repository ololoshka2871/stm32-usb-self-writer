use packing::Packed;
use crate::scsi::{
    packing::ParsePackedStruct,
    commands::Control,
};

// https://www.mikrocontroller.net/attachment/41812/0x23_READFMT_070123.pdf: Table 701
#[derive(Clone, Copy, Eq, PartialEq, Debug, Packed)]
#[packed(big_endian, lsb0)]
pub struct ReadFormatCapacitiesCommand {
    #[pkd(7, 0, 0, 0)]
    pub op_code: u8,

    #[pkd(7, 5, 1, 1)]
    pub logical_unit_number: u8,
    
    #[pkd(7, 0, 7, 8)]
    pub allocation_length: u16,
    
    #[pkd(7, 0, 9, 9)]
    pub control: Control,
}
impl ParsePackedStruct for ReadFormatCapacitiesCommand {}