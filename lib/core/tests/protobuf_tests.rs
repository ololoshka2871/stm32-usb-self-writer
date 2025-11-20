// Host tests for core protobuf helpers
use corelogic::protobuf;

struct MockStream {
    data: Vec<u8>,
    pos: usize,
}

impl MockStream {
    fn new(bytes: &[u8]) -> Self {
        Self { data: bytes.to_vec(), pos: 0 }
    }
}

impl<T> protobuf::stream::Stream<T> for MockStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<(), T> {
        if self.pos >= self.data.len() {
            return Err(unsafe { core::mem::zeroed() });
        }
        let n = core::cmp::min(buf.len(), self.data.len() - self.pos);
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(())
    }

    fn read_all(&mut self) -> Result<Vec<u8>, T> {
        let rest = self.data[self.pos..].to_vec();
        self.pos = self.data.len();
        Ok(rest)
    }
}

#[test]
fn decode_magick_and_size() {
    // construct stream: first magick byte, then varint length 5 (0x05)
    let bytes = [protobuf::messages::Info::Magick as u8, 0x05u8];
    let mut s = MockStream::new(&bytes);

    // decode_magick should succeed
    let res = protobuf::md::decode_magick::<(), MockStream>(&mut s);
    assert!(res.is_ok());

    // after decoding magick, decode_msg_size should read the next byte and return 5
    // reset stream to start at second byte for decode_msg_size
    let mut s2 = MockStream::new(&bytes[1..]);
    let sz = protobuf::md::decode_msg_size::<(), MockStream>(&mut s2).expect("decode size");
    assert_eq!(sz, 5usize);
}
