use core::fmt::Debug;

use prost::{DecodeError, Message};

use super::{ProtobufProcessInputError, Stream};

pub fn recive_message_body<T: Debug, S: Stream<T>>(
    stream: &mut S,
) -> Result<super::messages::Request, DecodeError> {
    let data = stream
        .read_all()
        .map_err(|_| DecodeError::new("Failed to read message body"))?;

    super::messages::Request::decode(data.as_slice())
}

//-----------------------------------------------------------------------------

pub async fn recive_message_body_async<E: Debug, S: super::AsyncStream<E>>(
    stream: &mut S,
    msg_size: usize,
) -> Result<super::messages::Request, ProtobufProcessInputError<E>> {
    let data = stream
        .read_size(msg_size)
        .await
        .map_err(ProtobufProcessInputError::from_input_error)?;

    super::messages::Request::decode(data.as_slice())
        .map_err(ProtobufProcessInputError::from_decode_error)
}
