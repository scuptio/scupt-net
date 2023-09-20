use std::net::SocketAddr;
use std::sync::Arc;

use scupt_util::error_type::ET;
use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use tokio::sync::oneshot;

use crate::endpoint::Endpoint;
use crate::event_channel::EventChannel;

pub enum EventResult {
    ErrorType(ET),
    NetEndpoint(Endpoint),
}

pub type ResultSender = oneshot::Sender<EventResult>;
pub type ResultReceiver = oneshot::Receiver<EventResult>;

pub enum NetEvent<
    M: MsgTrait + 'static,
> {
    NetConnect {
        node_id: NID,
        address: SocketAddr,
        opt_sender: Option<oneshot::Sender<EventResult>>,
        return_endpoint: bool,
    },
    NetListen(SocketAddr, Option<oneshot::Sender<EventResult>>),
    NetSend(NID, M, Option<oneshot::Sender<EventResult>>),

    Stop(Option<oneshot::Sender<EventResult>>),
    NewEventChannel(Arc<EventChannel<M>>),
}


