use core::fmt::Debug;

use alloc::{format, vec::Vec};

use prost::DecodeError;

use super::{AsyncStream, ProtobufProcessInputError, Stream};

pub fn recive_md_header<T: Debug, S: Stream<T>>(stream: &mut S) -> Result<usize, DecodeError> {
    decode_magick(stream)?;
    decode_msg_size(stream)
}

pub fn decode_magick<T: Debug, S: Stream<T>>(stream: &mut S) -> Result<(), DecodeError> {
    let mut v = [0u8];
    stream
        .read(&mut v)
        .map_err(|e| DecodeError::new(format!("{:?}", e)))?;
    if v[0] != super::messages::Info::Magick as u8 {
        Err(DecodeError::new(format!("Invalid magick: {}", v[0])))
    } else {
        Ok(())
    }
}

pub fn decode_msg_size<T: Debug, S: Stream<T>>(stream: &mut S) -> Result<usize, DecodeError> {
    let mut data = Vec::with_capacity(4);
    for _ in 0..data.capacity() {
        let mut b = [0u8];
        stream
            .read(&mut b)
            .map_err(|e| DecodeError::new(format!("{:?}", e)))?;
        data.push(b[0]);
        if b[0] < 0x80 {
            break;
        }
    }

    match prost::decode_length_delimiter(data.as_slice()) {
        Ok(v) => {
            if v == 0 || v > 1500 {
                Err(DecodeError::new(format!("Invalid message length {}", v)))
            } else {
                Ok(v)
            }
        }
        Err(e) => Err(e),
    }
}

//-----------------------------------------------------------------------------

pub async fn recive_md_header_async<E: Debug, S: AsyncStream<E>>(
    stream: &mut S,
) -> Result<usize, ProtobufProcessInputError<E>> {
    decode_magick_async(stream).await?;
    decode_msg_size_async(stream).await
}

pub async fn decode_magick_async<E: Debug, S: AsyncStream<E>>(
    stream: &mut S,
) -> Result<(), ProtobufProcessInputError<E>> {
    let mut v = [0u8];
    stream
        .read(&mut v)
        .await
        .map_err(ProtobufProcessInputError::from_input_error)?;
    if v[0] != super::messages::Info::Magick as u8 {
        Err(ProtobufProcessInputError::from_decode_error(
            DecodeError::new(format!("Invalid magick: {}", v[0])),
        ))
    } else {
        Ok(())
    }
}

pub async fn decode_msg_size_async<E: Debug, S: AsyncStream<E>>(
    stream: &mut S,
) -> Result<usize, ProtobufProcessInputError<E>> {
    let mut data = Vec::with_capacity(4);
    for _ in 0..data.capacity() {
        let mut b = [0u8];
        stream
            .read(&mut b)
            .await
            .map_err(ProtobufProcessInputError::from_input_error)?;
        data.push(b[0]);
        if b[0] < 0x80 {
            break;
        }
    }

    match prost::decode_length_delimiter(data.as_slice()) {
        Ok(v) => {
            if v == 0 || v > 1500 {
                Err(ProtobufProcessInputError::from_decode_error(
                    DecodeError::new(format!("Invalid message length {}", v)),
                ))
            } else {
                Ok(v)
            }
        }
        Err(e) => Err(ProtobufProcessInputError::from_decode_error(e)),
    }
}
