use std::net::SocketAddr;
use std::sync::Arc;

use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;

use crate::endpoint_sync::EndpointSync;
use crate::es_option::{ESConnectOpt, ESServeOpt, ESStopOpt};

pub trait EventSinkSync<M: MsgTrait + 'static>: Sync + Send {
    fn stop(&self, opt: ESStopOpt) -> Res<()>;

    fn serve(&self, addr: SocketAddr, opt: ESServeOpt) -> Res<()>;

    fn connect(&self, node_id: NID, address: SocketAddr, opt: ESConnectOpt) -> Res<Option<Arc<dyn EndpointSync<M>>>>;
}
