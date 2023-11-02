use core::slice::SlicePattern;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use scupt_util::error_type::ET;
use scupt_util::message::{decode_message, encode_message, MsgTrait};
use scupt_util::res::Res;
use scupt_util::res_of::res_io;

use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::codec::Framed;
use tracing::{error, Instrument, trace_span};

use crate::framed_codec::FramedCodec;
use crate::opt_ep::OptEP;
use crate::serde_json_string::ActionSerdeJsonString;


type FramedStream = Framed<TcpStream, FramedCodec>;

struct _Endpoint {
    framed: Mutex<FramedStream>,
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
        Self {
            framed: Mutex::new(framed),
            remote_address: address,
            enable_dtm_test
        }
    }

    fn remote_address(&self) -> SocketAddr {
        self.remote_address
    }

    // send message
    async fn send<M: MsgTrait + 'static>(&self, m: M) -> Res<()> {
        let vec = encode_message(m)?;
        let bytes = BytesMut::from(vec.as_slice());
        let mut framed = self.framed.lock().await;
        let r = framed.send(bytes).await;
        match r {
            Ok(_) => { Ok(()) }
            Err(_e) => { Err(ET::TokioSenderError("send network message error".to_string())) }
        }
    }

    // receive a message
    async fn recv<M: MsgTrait + 'static>(&self) -> Res<M> {
        let mut framed = self.framed.lock().instrument(trace_span!("lock")).await;
        let opt = framed.next().await;
        let r = match opt {
            Some(r) => { r }
            None => { return Err(ET::StopService); }
        };
        let b = match r {
            Ok(b) => { b }
            Err(_e) => { return Err(ET::NoneOption); }
        };
        let r = decode_message::<M>(b.as_slice());
        match r {
            Ok((m, _)) => { return Ok(m) }
            Err(e) => {
                if self.enable_dtm_test {
                    return parse_dtm_action_message(b.as_slice())
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn close(&self) -> Res<()> {
        let mut f = self.framed.lock().await;
        let r = f.close().await;
        res_io(r)
    }
}


fn parse_dtm_action_message<M:MsgTrait + 'static>(byte:&[u8]) -> Res<M> {
    let (json, _) = decode_message::<ActionSerdeJsonString>(byte)?;
    let string = json.to_string()?;
    let r = serde_json::from_str::<M>(string.as_str());
    match r {
        Ok(m) => { Ok(m)  }
        Err(e) => {
            error!("{}", e.to_string());
            serde_json::from_str::<M>(string.as_str()).unwrap();
            Err(ET::SerdeError(e.to_string()))
        }
    }
}

impl Endpoint {
    pub fn new(stream: TcpStream, remote_address: SocketAddr, opt_ep:OptEP) -> Self {
        Self {
            _ep: Arc::new(_Endpoint::new(stream, remote_address, opt_ep.is_enable_dtm_test())),
        }
    }

    pub async fn send<M: MsgTrait + 'static>(&self, m: M) -> Res<()> {
        self._ep.send(m).await
    }

    pub async fn recv<M: MsgTrait + 'static>(&self) -> Res<M> {
        self._ep.recv::<M>().await
    }

    pub fn remote_address(&self) -> SocketAddr {
        self._ep.remote_address()
    }

    pub async fn close(&self) -> Res<()> {
        self._ep.close().await
    }
}