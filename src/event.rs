use std::fmt::{Debug, Formatter};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::Receiver as _SyncReceiver;
use std::sync::mpsc::Sender as _SyncSender;

use scupt_util::message::{Message, MsgTrait};
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use tokio::sync::oneshot::Receiver as _AsyncReceiver;
use tokio::sync::oneshot::Sender as _AsyncSender;

use crate::endpoint_async::EndpointAsync;
use crate::endpoint_sync::EndpointSync;
use crate::event_channel::EventChannel;

pub type SyncSender<M> = _SyncSender<M>;
pub type SyncReceiver<M> = _SyncReceiver<M>;

pub type AsyncSender<M> = _AsyncSender<M>;
pub type AsyncReceiver<M> = _AsyncReceiver<M>;

pub enum ResultSenderType<S, A> {
    SendNone,
    Sync(SyncSender<S>),
    Async(AsyncSender<A>),
}

pub enum NetEvent<
    M: MsgTrait + 'static,
> {
    NetConnect {
        node_id: NID,
        return_endpoint: bool,
        address: SocketAddr,
        opt_sender: ResultSenderType<
            Res<Option<Arc<dyn EndpointSync<M>>>>,
            Res<Option<Arc<dyn EndpointAsync<M>>>>
        >,
    },
    NetListen(SocketAddr, ResultSenderType<
        Res<Option<Arc<dyn EndpointSync<M>>>>,
        Res<Option<Arc<dyn EndpointAsync<M>>>>
    >,
    ),
    NetSend(Message<M>, ResultSenderType<
        Res<Option<Arc<dyn EndpointSync<M>>>>,
        Res<Option<Arc<dyn EndpointAsync<M>>>>
    >),
    Stop(ResultSenderType<Res<()>, Res<()>>),
    NewEventChannel(Arc<EventChannel<M>>),
}

impl <M:MsgTrait + 'static> Debug for NetEvent<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NetEvent::NetConnect {
                node_id,
                return_endpoint,
                address,
                opt_sender: _
            } => {
                write!(f, "NetConnect({:?}, {:?} return endpoint: {:?})",
                       node_id, address, return_endpoint)?;
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
