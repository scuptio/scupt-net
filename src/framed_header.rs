use std::mem::size_of;

use byteorder;
use byteorder::{ByteOrder, NetworkEndian};

// message codec
// message
// 4 bytes message length (assume it is N)
// N bytes message payload

const HEADER_SIZE: usize = 1usize * size_of::<u32>();
const HEADER_BODY_SIZE_OFFSET: usize = 0;


// frame header with a reference to a slice buffer
pub struct FramedHdrRef<'a> {
    buf: &'a [u8],
}

// frame header
pub struct FramedHdr {
    buf: [u8; HEADER_SIZE],
}

impl<'a> FramedHdrRef<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        assert_eq!(buf.len(), Self::size());
        FramedHdrRef {
            buf,
        }
    }

    pub fn size() -> usize {
        HEADER_SIZE
    }

    pub fn get_size(&self) -> u32 {
        NetworkEndian::read_u32(&self.buf[HEADER_BODY_SIZE_OFFSET..])
    }
}

impl FramedHdr {
    pub fn size() -> usize {
        HEADER_SIZE
    }

    pub fn new() -> Self {
        Self {
            buf: [0; HEADER_SIZE]
        }
    }

    pub fn buf(&self) -> &[u8] {
        &self.buf
    }


    pub fn set_size(&mut self, value: u32) {
        NetworkEndian::write_u32(&mut self.buf[HEADER_BODY_SIZE_OFFSET..], value);
    }
}