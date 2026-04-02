use alloc::sync::Arc;
use alloc::vec::Vec;
use freertos_rust::{Duration, FreeRtosError, Mutex};
use self_recorder_packet::DataBlockPacker;

use crate::{
    settings,
    threads::sensor_processor::FChannel,
    workmodes::output_storage::OutputStorage,
};

use super::{
    data_page::DataPage,
    write_controller::{self, WriteController},
};

#[derive(Clone)]
pub struct FlashDiffWriter {
    next_page_number: u32,
    crc_calc: Arc<Mutex<stm32l4xx_hal::crc::Crc>>,
    fref_mul: f32,
    page_aqured: bool,
}

pub struct DataBlock {
    page_number: u32,
    page_size: usize,
    timestamp: u64,
    f_ref: f32,
    base_interval_ms: u32,
    interleave_ratio: [u32; 2],
    targets: [u32; 2],
    t_cpu: f32,
    v_bat: f32,

    samples: Vec<i32>,
    max_samples: usize,

    dest_page: u32,
    prevs: [u32; 2],
}

impl DataPage for DataBlock {
    fn write_header(&mut self, output: &OutputStorage) {
        self.targets = output.targets;
        self.t_cpu = output.t_cpu;
        self.v_bat = output.vbat;

        let h = self_recorder_packet::DataPacketHeader {
            prev_block_id: self.page_number.checked_sub(1).unwrap_or_default(),
            this_block_id: self.page_number,

            timestamp: self.timestamp,
            f_ref: self.f_ref,
            targets: self.targets,

            base_interval_ms: self.base_interval_ms,
            interleave_ratio: self.interleave_ratio,

            t_cpu: self.t_cpu,
            v_bat: self.v_bat,

            data_len: 0,
            data_crc32: 0,
        };

        defmt::debug!(
            "{}",
            crate::main_data_storage::header_printer::HeaderPrinter(&h)
        );
    }

    fn push_data(&mut self, result: Option<u32>, channel: FChannel) -> bool {
        defmt::trace!("DataPage::push_data(result={}, ch={})", result, channel);
        let v = if let Some(r) = result {
            #[allow(unnecessary_transmutes)] // modern rust warning
            let diff = r as i32
                - unsafe { core::mem::transmute::<u32, i32>(self.prevs[channel as usize]) };
            self.prevs[channel as usize] = r;
            diff
        } else {
            0
        };
        self.samples.push(v);
        self.samples.len() >= self.max_samples
    }

    fn finalise(&mut self) {
        //defmt::debug!("DataPage::finalise()");
    }
}

impl FlashDiffWriter {
    pub fn new(fref_mul: f32, crc_calc: Arc<Mutex<stm32l4xx_hal::crc::Crc>>) -> Self {
        Self {
            next_page_number: 0,
            crc_calc,
            fref_mul: fref_mul,
            page_aqured: false,
        }
    }
}

impl WriteController<DataBlock> for FlashDiffWriter {
    fn try_create_new_page(&mut self, page_number: u32) -> Result<DataBlock, FreeRtosError> {
        if !self.page_aqured {
            if let Some(ep) = crate::main_data_storage::find_next_empty_page(self.next_page_number)
            {
                defmt::info!("Aquaering page {}", ep);
                self.next_page_number = ep;
            } else {
                defmt::error!("Aquaering page failed, memory full!");
                return Err(freertos_rust::FreeRtosError::OutOfMemory);
            }

            let (base_interval_ms, interleave_ratio, fref) =
                match settings::settings_action::<_, _, _, ()>(Duration::ms(10), |(settings, _)| {
                    Ok((
                        settings.writeConfig.BaseInterval_ms,
                        [
                            settings.writeConfig.PWriteDevider,
                            settings.writeConfig.TWriteDevider,
                        ],
                        settings.Fref,
                    ))
                }) {
                    Ok(r) => r,
                    Err(settings::SettingActionError::AccessError(e)) => return Err(e),
                    _ => unreachable!(),
                };

            let page_size = crate::main_data_storage::flash_page_size() as usize;
            let header_size = core::mem::size_of::<self_recorder_packet::DataPacketHeader>();
            let max_samples = (page_size.saturating_sub(header_size + 128) / core::mem::size_of::<i32>())
                .max(1);

            let res = DataBlock {
                page_number,
                page_size,
                timestamp: crate::rtc::rtc_get_time().to_timestamp_ms(),
                f_ref: self.fref_mul * fref as f32,
                base_interval_ms,
                interleave_ratio,
                targets: [0; 2],
                t_cpu: 0.0,
                v_bat: 0.0,
                samples: Vec::with_capacity(max_samples),
                max_samples,
                dest_page: self.next_page_number,
                prevs: [0, 0],
            };

            self.next_page_number += 1;
            self.page_aqured = true;

            Ok(res)
        } else {
            Err(freertos_rust::FreeRtosError::OutOfMemory)
        }
    }

    fn write(&mut self, page: DataBlock) -> write_controller::PageWriteResult {
        let id = page.page_number;
        let input_count = page.samples.len();

        let mut packer = DataBlockPacker::builder()
            .set_ids(page.page_number.checked_sub(1).unwrap_or_default(), page.page_number)
            .set_size(page.page_size)
            .set_timestamp(page.timestamp)
            .set_fref(page.f_ref)
            .set_write_cfg(page.base_interval_ms, page.interleave_ratio)
            .set_targets(page.targets)
            .set_tcpu(page.t_cpu)
            .set_vbat(page.v_bat)
            .build();

        for sample in page.samples.iter().copied() {
            match packer.push_val(sample) {
                self_recorder_packet::PushResult::Success => {}
                self_recorder_packet::PushResult::Full => break,
                self_recorder_packet::PushResult::Overflow => {
                    defmt::error!("Page {} compression overflow", id);
                    self.page_aqured = false;
                    return write_controller::PageWriteResult::Fail(id);
                }
                self_recorder_packet::PushResult::Finished => break,
            }
        }

        self.page_aqured = false;
        if let Some(data) = packer.to_result_full(|data| {
            self.crc_calc
                .lock(Duration::infinite())
                .map(|mut crc_guard| {
                    crc_guard.reset();
                    crc_guard.feed(data);
                    !crc_guard.result() // результат инвертируется, чтобы соотвектсвовать zlib
                })
                .unwrap_or_default()
        }) {
            if let Ok(mut page_accessor) = crate::main_data_storage::select_page(page.dest_page) {
                let len = data.len();
                if let Ok(()) = page_accessor.write(data.as_slice()) {
                    defmt::info!(
                        "Write page {}, {} values ({} bytes) -> {}",
                        id,
                        input_count,
                        input_count * core::mem::size_of::<u32>(),
                        len
                    );
                    return write_controller::PageWriteResult::Succes(id);
                }
            }

            defmt::error!("Failed to get page {}!", page.dest_page);
            return write_controller::PageWriteResult::Fail(id);
        } else {
            defmt::error!("Page {} generation failed!", id);
            write_controller::PageWriteResult::Fail(id)
        }
    }
}
