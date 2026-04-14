use core::fmt::Debug;

mod change_password;
mod device_info;
mod encode_md_message;
mod md;
mod message_body;
mod messages;
mod monitoring_over_conditions;
mod new_response;
mod output;
mod process_request;
mod process_settings;
mod stream;

pub use encode_md_message::encode_md_message;
pub use md::{recive_md_header, recive_md_header_async};
pub use message_body::{recive_message_body, recive_message_body_async};
pub use new_response::new_response;
pub use process_request::process_request;
pub use stream::{AsyncStream, Stream};

pub use messages::{P_COEFFS_COUNT, PASSWORD_SIZE, Response, T_COEFFS_COUNT};

#[derive(Debug)]
pub enum ProtobufProcessInputError<IE: Debug> {
    Input(IE),
    Decode(prost::DecodeError),
}

impl<IE: Debug> ProtobufProcessInputError<IE> {
    pub fn from_input_error(e: IE) -> Self {
        ProtobufProcessInputError::Input(e)
    }

    pub fn from_decode_error(e: prost::DecodeError) -> Self {
        ProtobufProcessInputError::Decode(e)
    }
}

#[derive(Debug, Default)]
pub enum ProtobufProcessError<IE: Debug, OE: Debug> {
    #[default]
    Ok,
    Input(IE),
    Output(OE),
    Decode(prost::DecodeError),
    Encode(prost::EncodeError),
}

impl<IE: Debug, OE: Debug> ProtobufProcessError<IE, OE> {
    pub fn from_output_error(e: OE) -> Self {
        ProtobufProcessError::Output(e)
    }

    pub fn from_encode_error(e: prost::EncodeError) -> Self {
        ProtobufProcessError::Encode(e)
    }
}

impl<IE: Debug, OE: Debug> From<ProtobufProcessInputError<IE>> for ProtobufProcessError<IE, OE> {
    fn from(e: ProtobufProcessInputError<IE>) -> Self {
        match e {
            ProtobufProcessInputError::Input(e) => ProtobufProcessError::Input(e),
            ProtobufProcessInputError::Decode(e) => ProtobufProcessError::Decode(e),
        }
    }
}

impl<IE: Debug, OE: Debug> defmt::Format for ProtobufProcessError<IE, OE> {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            ProtobufProcessError::Ok => defmt::write!(fmt, "Ok"),
            ProtobufProcessError::Input(e) => {
                defmt::write!(fmt, "Input error: {:?}", defmt::Debug2Format(e))
            }
            ProtobufProcessError::Output(e) => {
                defmt::write!(fmt, "Output error: {:?}", defmt::Debug2Format(e))
            }
            ProtobufProcessError::Decode(e) => {
                defmt::write!(fmt, "Decode error: {:?}", defmt::Debug2Format(e))
            }
            ProtobufProcessError::Encode(e) => {
                defmt::write!(fmt, "Encode error: {:?}", defmt::Debug2Format(e))
            }
        }
    }
}
