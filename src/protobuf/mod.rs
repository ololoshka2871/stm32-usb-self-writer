mod change_password;
mod device_info;
mod monitoring_over_conditions;
mod new_response;
mod output;
mod process_requiest;
mod process_settings;

pub use corelogic::protobuf::messages;

pub use corelogic::protobuf::encode_md_message::encode_md_message;
pub use corelogic::protobuf::md::recive_md_header;
pub use corelogic::protobuf::message_body::recive_message_body;
pub use new_response::new_response;
pub use process_requiest::process_requiest;
pub use corelogic::protobuf::stream::Stream;
pub use corelogic::protobuf::messages::Validator;

pub use messages::{Response, PASSWORD_SIZE};

use freertos_rust::Duration;

lazy_static::lazy_static! {
    pub static ref OUT_STORAGE_LOCK_WAIT: Duration = Duration::ms(5);
}