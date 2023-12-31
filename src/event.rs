use std::fmt::{Debug, Formatter};
use std::net::SocketAddr;
use std::sync::Arc;

use scupt_util::error_type::ET;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use tokio::sync::oneshot;

use crate::endpoint::Endpoint;
use crate::event_channel::EventChannel;

pub enum EventResult {
    ErrorType(ET),
    NetEndpoint(Res<Endpoint>),
}

pub type ResultSender = oneshot::Sender<EventResult>;
pub type ResultReceiver = oneshot::Receiver<EventResult>;

pub struct NetSendControl {
    pub sender: Option<oneshot::Sender<EventResult>>,
    pub return_response: bool
}

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
    NetSend(Message<M>, NetSendControl),

    Stop(Option<oneshot::Sender<EventResult>>),
    NewEventChannel(Arc<EventChannel<M>>),
}

impl <M:MsgTrait + 'static> Debug for NetEvent<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NetEvent::NetConnect { node_id, address, opt_sender, return_endpoint } => {
                write!(f, "NetConnect({:?}, {:?}, opt_sender_not_none:{}, return_endpoint{})",
                       node_id, address, opt_sender.is_some(), return_endpoint, )?;
            }
            NetEvent::NetListen(address, _) => {
                write!(f, "NetListen({:?})", address)?;
            }
            NetEvent::NetSend( m, _) => {
                write!(f, "NetSend({:?})", m)?;
            }
            NetEvent::Stop(_) => {
                write!(f, "Stop(_)")?;
            }
            NetEvent::NewEventChannel(_) => {
                write!(f, "NewEventChannel(_)")?;
            }
        }
        Ok(())
    }
}
