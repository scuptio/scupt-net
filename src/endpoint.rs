use core::slice::SlicePattern;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use futures::stream::{SplitSink, SplitStream};
use scupt_util::error_type::ET;
use scupt_util::message::{decode_message, encode_message, Message, MsgTrait};
use scupt_util::res::Res;
use scupt_util::res_of::res_io;

use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::codec::Framed;
use tracing::{Instrument, trace_span};

use crate::framed_codec::FramedCodec;
use crate::opt_ep::OptEP;
use crate::parse_dtm_message;

struct _Endpoint {
    sender:Mutex<SplitSink<Framed<TcpStream, FramedCodec>, BytesMut>>,
    receiver:Mutex<SplitStream<Framed<TcpStream, FramedCodec>>>,
    remote_address: SocketAddr,
    // is enable DTM testing, default is false
    // when this option was enabling, the incoming message would be parse as ActionMessage
    enable_dtm_test:bool
}

#[derive(Clone)]
pub struct Endpoint {
    _ep: Arc<_Endpoint>,
}

impl _Endpoint {
    fn new(stream: TcpStream, address: SocketAddr,
        enable_dtm_test:bool
    ) -> Self {
        let framed = Framed::new(
            stream,
            FramedCodec::new()
        );
        let (s, r) = framed.split();
        Self {
            sender: Mutex::new(s),
            receiver : Mutex::new(r),
            remote_address: address,
            enable_dtm_test
        }
    }

    fn remote_address(&self) -> SocketAddr {
        self.remote_address
    }

    // send message
    async fn send<M: MsgTrait + 'static>(&self, m: Message<M>) -> Res<()> {
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
    async fn recv<M: MsgTrait + 'static>(&self) -> Res<Message<M>> {
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
            Ok((m, _)) => { return Ok(m) }
            Err(e) => {
                if self.enable_dtm_test {
                    return parse_dtm_message::parse_dtm_message(b.as_slice())
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn close(&self) -> Res<()> {
        let r1 = {
            let mut sink = self.sender.lock().await;
            sink.close().await
        };
        res_io(r1)?;
        Ok(())
    }
}

impl Endpoint {
    pub fn new(stream: TcpStream, remote_address: SocketAddr, opt_ep:OptEP) -> Self {
        Self {
            _ep: Arc::new(_Endpoint::new(stream, remote_address, opt_ep.is_enable_dtm_test())),
        }
    }

    pub async fn send<M: MsgTrait + 'static>(&self, m: Message<M>) -> Res<()> {
        self._ep.send(m).await
    }

    pub async fn recv<M: MsgTrait + 'static>(&self) -> Res<Message<M>> {
        self._ep.recv::<M>().await
    }

    pub fn remote_address(&self) -> SocketAddr {
        self._ep.remote_address()
    }

    pub async fn close(&self) -> Res<()> {
        self._ep.close().await
    }
}