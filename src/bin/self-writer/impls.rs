use core::fmt::Debug;

use alloc::{boxed::Box, vec::Vec};

use rtic_sync::channel::{NoReceiver, ReceiveError, Receiver, Sender};
use stm32_usb_self_writer::protobuf::{self, AsyncStream};

pub struct AsyncProtobufStream<'a, const N: usize> {
    receiver: &'a mut Receiver<'static, Vec<u8>, N>,
    working_buffer: Vec<u8>,
}

impl<'a, const N: usize> AsyncProtobufStream<'a, N> {
    pub fn new(receiver: &'a mut Receiver<'static, Vec<u8>, N>) -> Self {
        Self {
            receiver,
            working_buffer: Vec::new(),
        }
    }

    async fn get_buffer(&mut self) -> Result<&mut Vec<u8>, ReceiveError> {
        if self.working_buffer.is_empty() {
            match self.receiver.recv().await {
                Ok(data) => self.working_buffer = data,
                Err(e) => return Err(e),
            }
        }
        Ok(&mut self.working_buffer)
    }
}

#[async_trait::async_trait]
impl<'a, const N: usize> AsyncStream<ReceiveError> for AsyncProtobufStream<'a, N> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<(), ReceiveError> {
        let buffer = self.get_buffer().await?;
        if buf.len() > buffer.len() {
            let avalable = buffer.len();
            buf.copy_from_slice(&buffer[..avalable]);
            buffer.clear();
            self.read(&mut buf[avalable..]).await
        } else {
            buf.copy_from_slice(&buffer[..buf.len()]);
            buffer.drain(..buf.len());
            Ok(())
        }
    }

    async fn read_size(&mut self, size: usize) -> Result<Vec<u8>, ReceiveError> {
        let mut result = Vec::with_capacity(size);
        while result.len() < size {
            let buffer = self.get_buffer().await?;
            let to_take = core::cmp::min(size - result.len(), buffer.len());
            result.extend_from_slice(&buffer[..to_take]);
            buffer.drain(..to_take);
        }
        Ok(result)
    }
}

pub async fn process_protobuf<IE: Debug, IS: AsyncStream<IE>, const N: usize>(
    input_stream: &mut IS,
    output: &mut Sender<'static, Vec<u8>, N>,
) -> Result<(), protobuf::ProtobufProcessError<IE, NoReceiver<Vec<u8>>>> {
    let msg_size = protobuf::recive_md_header_async(input_stream)
        .await?;
    let request = protobuf::recive_message_body_async(input_stream, msg_size).await?;

    defmt::trace!("Received request: {:?}", defmt::Debug2Format(&request));

    Ok(())
}
