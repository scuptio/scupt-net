use core::slice::SlicePattern;
use std::io;

use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::framed_header::{FramedHdr, FramedHdrRef};

/// A simple [`Decoder`] and [`Encoder`] implementation that just ships bytes around.
///
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct FramedCodec(());

impl FramedCodec {
    /// Creates a new `BytesCodec` for shipping around raw bytes.
    pub fn new() -> FramedCodec {
        FramedCodec(())
    }
}

impl Decoder for FramedCodec {
    type Item = BytesMut;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, io::Error> {
        if buf.len() <= FramedHdr::size() {
            return Ok(None);
        } else {
            // retrieve the header first, and get the message size
            let hdr = FramedHdrRef::new(buf[0..FramedHdrRef::size()].as_slice());
            let msg_size = hdr.get_size() as usize;
            if buf.len() >= msg_size + FramedHdrRef::size() {
                // have a full message
                buf.advance(FramedHdrRef::size());
                Ok(Some(buf.split_to(msg_size)))
            } else {
                return Ok(None);
            }
        }
    }
}


impl Encoder<BytesMut> for FramedCodec {
    type Error = io::Error;

    fn encode(&mut self, data: BytesMut, buf: &mut BytesMut) -> Result<(), io::Error> {
        let mut header = FramedHdr::new();
        header.set_size(data.len() as u32);
        buf.reserve(FramedHdr::size() + data.len());
        // write the header first
        buf.put(header.buf());
        // write the message
        buf.put(data);
        Ok(())
    }
}
