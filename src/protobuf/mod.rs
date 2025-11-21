mod change_password;
mod device_info;
mod monitoring_over_conditions;
mod output;
mod process_request;
mod process_settings;

pub use corelogic::protobuf::messages::{self, Response, Validator, PASSWORD_SIZE};

pub use corelogic::protobuf::encode_md_message::encode_md_message;
pub use corelogic::protobuf::md::recive_md_header;
pub use corelogic::protobuf::message_body::recive_message_body;
pub use corelogic::protobuf::stream::Stream;
pub use process_request::process_request;

use freertos_rust::Duration;

use corelogic::output_provider::OutputProvider;

use crate::workmodes::output_storage::OutputStorage;
use alloc::sync::Arc;
use freertos_rust::Mutex;

lazy_static::lazy_static! {
    pub static ref OUT_STORAGE_LOCK_WAIT: Duration = Duration::ms(5);
}

pub struct ArcOutputProvider(pub Arc<Mutex<OutputStorage>>);

impl OutputProvider for ArcOutputProvider {
    type Output = OutputStorage;
    type Error = freertos_rust::FreeRtosError;

    fn with_output<R>(&self, f: impl FnOnce(&Self::Output) -> R) -> Result<R, Self::Error> {
        match self.0.lock(*OUT_STORAGE_LOCK_WAIT) {
            Ok(guard) => Ok(f(&*guard)),
            Err(e) => Err(e),
        }
    }
}
