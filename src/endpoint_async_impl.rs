use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;
use tokio::net::TcpStream;

use crate::endpoint_async::EndpointAsync;
use crate::endpoint_inner::_Endpoint;
use crate::opt_ep::OptEP;

#[derive(Clone)]
pub struct EndpointAsyncImpl {
    _ep: Arc<_Endpoint>,
}


#[async_trait]
impl<M: MsgTrait + 'static> EndpointAsync<M> for EndpointAsyncImpl {
    fn remote_address(&self) -> SocketAddr {
        self._remote_address()
    }

    async fn send(&self, m: Message<M>) -> Res<()> {
        self._send(m).await
    }

    async fn recv(&self) -> Res<Message<M>> {
        self._recv().await
    }

    async fn close(&self) -> Res<()> {
        self._close().await
    }
}

impl EndpointAsyncImpl {
    pub fn new(stream: TcpStream, remote_address: SocketAddr, opt_ep: OptEP) -> Self {
        Self {
            _ep: Arc::new(_Endpoint::new(stream, remote_address, opt_ep.is_enable_dtm_test())),
        }
    }

    async fn _send<M: MsgTrait + 'static>(&self, m: Message<M>) -> Res<()> {
        self._ep.send(m).await
    }

    async fn _recv<M: MsgTrait + 'static>(&self) -> Res<Message<M>> {
        self._ep.recv::<M>().await
    }

    fn _remote_address(&self) -> SocketAddr {
        self._ep.remote_address()
    }

    async fn _close(&self) -> Res<()> {
        self._ep.close().await
    }
}
