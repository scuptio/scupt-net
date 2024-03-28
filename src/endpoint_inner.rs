use core::slice::SlicePattern;
use std::net::SocketAddr;

use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use futures::stream::{SplitSink, SplitStream};
use scupt_util::error_type::ET;
use scupt_util::message::{
    decode_message,
    encode_message,
    Message,
    MsgTrait,
};
use scupt_util::res::Res;
use scupt_util::res_of::res_io;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::codec::Framed;
use tracing::{Instrument, trace_span};

use crate::{parse_dtm_message, task_trace};
use crate::framed_codec::FramedCodec;

pub struct _Endpoint {
    sender: Mutex<SplitSink<Framed<TcpStream, FramedCodec>, BytesMut>>,
    receiver: Mutex<SplitStream<Framed<TcpStream, FramedCodec>>>,
    remote_address: SocketAddr,
    // is enabled DTM testing, default is false
    // when this option was enabling, the incoming message would be parse as ActionMessage
    enable_dtm_test: bool,
}

impl _Endpoint {
    pub fn new(stream: TcpStream, address: SocketAddr,
               enable_dtm_test: bool,
    ) -> Self {
        let framed = Framed::new(
            stream,
            FramedCodec::new(),
        );
        let (s, r) = framed.split();
        Self {
            sender: Mutex::new(s),
            receiver: Mutex::new(r),
            remote_address: address,
            enable_dtm_test,
        }
    }

    pub fn remote_address(&self) -> SocketAddr {
        self.remote_address
    }

    // send message
    #[async_backtrace::framed]
    pub async fn send<M: MsgTrait + 'static>(&self, m: Message<M>) -> Res<()> {
        let _t = task_trace!();
        if self.enable_dtm_test {
            return Ok(());
        }
        let vec = encode_message(m)?;
        let bytes = BytesMut::from(vec.as_slice());
        let mut sink = self.sender.lock().await;
        let r = sink.send(bytes).await;
        match r {
            Ok(_) => { Ok(()) }
            Err(_e) => { Err(ET::TokioSenderError("send network message error".to_string())) }
        }
    }

    // receive a message
    #[async_backtrace::framed]
    pub async fn recv<M: MsgTrait + 'static>(&self) -> Res<Message<M>> {
        let _t = task_trace!();

        let mut stream = self.receiver.lock().instrument(trace_span!("lock")).await;
        let opt = stream.next().await;
        let r = match opt {
            Some(r) => { r }
            None => { return Err(ET::EOF); }
        };
        let b = match r {
            Ok(b) => { b }
            Err(_e) => { return Err(ET::NoneOption); }
        };
        let r = decode_message::<Message<M>>(b.as_slice());
        match r {
            Ok((m, _)) => { return Ok(m); }
            Err(e) => {
                if self.enable_dtm_test {
                    return parse_dtm_message::parse_dtm_message(b.as_slice());
                } else {
                    Err(e)
                }
            }
        }
    }

    #[async_backtrace::framed]
    pub async fn close(&self) -> Res<()> {
        let _t = task_trace!();
        let r1 = {
            let mut sink = self.sender.lock().await;
            sink.close().await
        };
        res_io(r1)?;
        Ok(())
    }
}