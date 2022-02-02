mod qspi_driver;

use alloc::sync::Arc;
use freertos_rust::{Duration, FreeRtosError, Mutex};
use self_recorder_packet::DataBlockPacker;

use crate::{
    main_data_storage::PageAccessor,
    sensors::freqmeter::master_counter::{MasterCounter, MasterTimerInfo},
    settings,
    threads::sensor_processor::FChannel,
    workmodes::output_storage::OutputStorage,
};

use super::{
    data_page::DataPage,
    write_controller::{self, WriteController},
};

pub struct CpuFlashDiffWriter {
    master_counter_info: MasterTimerInfo,
    next_page_number: u32,
    crc_calc: Arc<Mutex<stm32l4xx_hal::crc::Crc>>,
    fref_mul: f32,
    page_aqured: bool,
}

pub struct DataBlock {
    packer: DataBlockPacker,
    counter: usize,
    dest_page: u32,
    prevs: [u32; 2],
}

impl DataPage for DataBlock {
    fn write_header(&mut self, output: &OutputStorage) {
        let h = &mut self.packer.header;

        h.targets = output.targets;
        h.t_cpu = output.t_cpu;
        h.v_bat = output.vbat;

        defmt::debug!(
            "{}",
            crate::main_data_storage::header_printer::HeaderPrinter(h)
        );
    }

    fn push_data(&mut self, result: Option<u32>, channel: FChannel) -> bool {
        defmt::trace!("DataPage::push_data(result={}, ch={})", result, channel);
        let v = if let Some(r) = result {
            let diff = r as i32
                - unsafe { core::mem::transmute::<u32, i32>(self.prevs[channel as usize]) };
            self.prevs[channel as usize] = r;
            diff
        } else {
            0
        };
        self.counter += 1;
        match self.packer.push_val(v) {
            self_recorder_packet::PushResult::Success => false,
            self_recorder_packet::PushResult::Full => true,
            self_recorder_packet::PushResult::Overflow => false,
            self_recorder_packet::PushResult::Finished => unreachable!(),
        }
    }

    fn finalise(&mut self) {
        //defmt::debug!("DataPage::finalise()");
    }
}

impl CpuFlashDiffWriter {
    pub fn new(fref_mul: f32, crc_calc: Arc<Mutex<stm32l4xx_hal::crc::Crc>>) -> Self {
        let mut master_counter_info = MasterCounter::acquire();
        master_counter_info.want_start();

        Self {
            master_counter_info,
            next_page_number: 0,
            crc_calc,
            fref_mul: fref_mul,
            page_aqured: false,
        }
    }
}

impl WriteController<DataBlock> for CpuFlashDiffWriter {
    fn try_create_new_page(&mut self) -> Result<DataBlock, FreeRtosError> {
        Err(FreeRtosError::OutOfMemory)
    }

    fn write(&mut self, page: DataBlock) -> write_controller::PageWriteResult {
        write_controller::PageWriteResult::Fail(0)
    }
}
