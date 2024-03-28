use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;

use crate::endpoint_async::EndpointAsync;
use crate::es_option::{ESConnectOpt, ESServeOpt, ESStopOpt};

#[async_trait]
pub trait EventSinkAsync<M: MsgTrait + 'static>: Sync + Send {
    async fn stop(&self, opt: ESStopOpt) -> Res<()>;

    async fn serve(&self, addr: SocketAddr, opt: ESServeOpt) -> Res<()>;

    async fn connect(&self, node_id: NID, address: SocketAddr, opt: ESConnectOpt) -> Res<Option<Arc<dyn EndpointAsync<M>>>>;
}

