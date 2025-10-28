use packing::{
    Packed,
    PackedSize,
};
use crate::scsi::enums::MediumType;

#[derive(Clone, Copy, Eq, PartialEq, Debug, Packed)]
#[packed(big_endian, lsb0)]
pub struct ModeParameterHeader6 {
    #[pkd(7, 0, 0, 0)]
    pub mode_data_length: u8,

    #[pkd(7, 0, 1, 1)]
    pub medium_type: MediumType,

    #[pkd(7, 0, 2, 2)]
    pub device_specific_parameter: SbcDeviceSpecificParameter,

    #[pkd(7, 0, 3, 3)]
    pub block_descriptor_length: u8,
}
impl Default for ModeParameterHeader6 {
    fn default() -> Self {
        Self {
            mode_data_length: Self::BYTES as u8 - 1,
            medium_type: Default::default(),
            device_specific_parameter: Default::default(),
            block_descriptor_length: 0,
        }
    }
}
impl ModeParameterHeader6 {
    /// Increase the relevant length fields to indicate the provided page follows this header
    /// can be called multiple times but be aware of the max length allocated by CBW
    pub fn increase_length_for_page(&mut self, page_code: PageCode) {
        self.mode_data_length += match page_code {
            PageCode::CachingModePage => CachingModePage::BYTES as u8,
        };
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Packed)]
#[packed(big_endian, lsb0)]
pub struct ModeParameterHeader10 {
    #[pkd(7, 0, 0, 1)]
    pub mode_data_length: u16,

    #[pkd(7, 0, 2, 2)]
    pub medium_type: MediumType,

    #[pkd(7, 0, 3, 3)]
    pub device_specific_parameter: SbcDeviceSpecificParameter,

    #[pkd(0, 0, 4, 4)]
    pub long_lba: bool,

    #[pkd(7, 0, 6, 7)]
    pub block_descriptor_length: u16,
}
impl Default for ModeParameterHeader10 {
    fn default() -> Self {
        Self {
            mode_data_length: Self::BYTES as u16 - 2,
            medium_type: Default::default(),
            device_specific_parameter: Default::default(),
            long_lba: Default::default(),
            block_descriptor_length: 0,
        }
    }
}
impl ModeParameterHeader10 {
    /// Increase the relevant length fields to indicate the provided page follows this header
    /// can be called multiple times but be aware of the max length allocated by CBW
    #[allow(dead_code)]
    pub fn increase_length_for_page(&mut self, page_code: PageCode) {
        self.mode_data_length += match page_code {
            PageCode::CachingModePage => CachingModePage::BYTES as u16,
        };
    }
}


#[derive(Clone, Copy, Eq, PartialEq, Debug, Packed, Default)]
#[packed(big_endian, lsb0)]
pub struct SbcDeviceSpecificParameter {
    #[pkd(7, 7, 0, 0)]
    pub write_protect: bool,

    #[pkd(4, 4, 0, 0)]
    pub disable_page_out_and_force_unit_access_available: bool,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Packed)]
pub enum PageCode {
    CachingModePage = 0x08,
}

/// https://www.seagate.com/files/staticfiles/support/docs/manual/Interface%20manuals/100293068j.pdf
/// p. 5.9.3
#[derive(Clone, Copy, Eq, PartialEq, Debug, Packed)]
#[packed(big_endian, lsb0)]
pub struct CachingModePage {
    #[pkd(7, 7, 0, 0)]
    pub ps: bool,

    #[pkd(6, 6, 0, 0)]
    pub spf: bool,

    #[pkd(5, 0, 0, 0)]
    pub page_ode: PageCode,

    #[pkd(7, 0, 1, 1)]
    pub page_length: u8,

    #[pkd(7, 7, 2, 2)]
    pub ic: bool,

    #[pkd(6, 6, 2, 2)]
    pub abpf: bool,

    #[pkd(5, 5, 2, 2)]
    pub cap: bool,

    #[pkd(4, 4, 2, 2)]
    pub disc: bool,

    #[pkd(3, 3, 2, 2)]
    pub size: bool,

    #[pkd(2, 2, 2, 2)]
    pub wce: bool,

    #[pkd(1, 1, 2, 2)]
    pub mf: bool,

    #[pkd(0, 0, 2, 2)]
    pub rcd: bool,

    #[pkd(7, 4, 3, 3)]
    pub demand_read_retention_priority: u8,

    #[pkd(3, 0, 3, 3)]
    pub write_retention_priority: u8,

    #[pkd(7, 0, 4, 5)]
    pub disable_prefetch_transfer_length: u16,

    #[pkd(7, 0, 6, 7)]
    pub minimum_prefetch: u16,

    #[pkd(7, 0, 8, 9)]
    pub maximum_prefetch: u16,

    #[pkd(7, 0, 10, 11)]
    pub maximum_prefetch_ceiling: u16,

    #[pkd(7, 7, 12, 12)]
    pub fsw: bool,

    #[pkd(6, 6, 12, 12)]
    pub lbcss: bool,

    #[pkd(5, 5, 12, 12)]
    pub dra: bool,

    #[pkd(4, 3, 12, 12)]
    pub vendor_specific: u8,

    #[pkd(2, 1, 12, 12)]
    pub sync_prog: u8,

    #[pkd(0, 0, 12, 12)]
    pub nv_dis: bool,

    #[pkd(7, 0, 13, 13)]
    pub number_of_cache_segments: u8,

    #[pkd(7, 0, 14, 15)]
    pub cache_segment_size: u16,

    #[pkd(7, 0, 16, 16)]
    pub reserved: u8,

    #[pkd(7, 0, 17, 19)]
    pub obsolete: u32,
}
impl Default for CachingModePage {
    fn default() -> Self {
        Self {
            ps: false,
            spf: false,
            page_ode: PageCode::CachingModePage,
            page_length: 0x12, // see table
            ic: false,
            abpf: false,
            cap: false,
            disc: false,
            size: false,
            wce: false,
            mf: false,
            rcd: false,
            demand_read_retention_priority: 0,
            write_retention_priority: 0,
            disable_prefetch_transfer_length: 0,
            minimum_prefetch: 0,
            maximum_prefetch: 0,
            maximum_prefetch_ceiling: 0,
            fsw: false,
            lbcss: false,
            dra: false,
            vendor_specific: 0,
            sync_prog: 0,
            nv_dis: false,
            number_of_cache_segments: 0,
            cache_segment_size: 0,
            reserved: 0,
            obsolete: 0,
        }
    }
}

// https://www.mikrocontroller.net/attachment/41812/0x23_READFMT_070123.pdf: Table 704
#[derive(Clone, Copy, Eq, PartialEq, Debug, Packed)]
#[packed(big_endian, lsb0)]
pub struct CurrentCapacityDescriptor {
    #[pkd(7, 0, 0, 3)]
    pub number_of_blocks: u32,

    #[pkd(7, 2, 4, 4)]
    pub reserved: u8,

    #[pkd(1, 0, 4, 4)]
    pub descriptor_type: u8,

    #[pkd(7, 0, 5, 8)]
    pub block_length: u32,
}

// https://www.mikrocontroller.net/attachment/41812/0x23_READFMT_070123.pdf: Table 701
#[derive(Clone, Copy, Eq, PartialEq, Debug, Packed)]
#[packed(big_endian, lsb0)]
pub struct FormatingCapacities {
    #[pkd(7, 0, 0, 2)]
    pub reserved: u32, //remove

    #[pkd(7, 0, 3, 3)]
    pub list_length: u8,

    #[pkd(7, 0, 4, 13)]
    pub descriptor: CurrentCapacityDescriptor,
}
impl FormatingCapacities {
    pub fn new(block_count: u32, block_length: usize) -> Self {
        Self {
            reserved: 0,
            list_length: CurrentCapacityDescriptor::BYTES as u8,
            descriptor: CurrentCapacityDescriptor {
                number_of_blocks: block_count,
                reserved: 0,
                descriptor_type: 0b10,
                block_length: block_length as u32,
            },
        }
    }
}
