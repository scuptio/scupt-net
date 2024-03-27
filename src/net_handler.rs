use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bincode::{Decode, Encode};
use scupt_util::error_type::ET;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tracing::{error, Instrument, trace, trace_span};

use crate::endpoint_async::EndpointAsync;
use crate::endpoint_sync::EndpointSync;
use crate::endpoint_sync_impl::EndpointSyncImpl;
use crate::event_sink_async_impl::EventSenderImpl;
use crate::handle_event::HandleEvent;
use crate::message_channel::{MessageChAsyncSender, MessageChSyncSender};
use crate::message_receiver_channel_async::MessageReceiverChannelAsync;
use crate::message_receiver_channel_sync::MessageReceiverChannelSync;
use crate::notifier::Notifier;
use crate::task::spawn_local_task;

pub type NodeSender<MsgTrait> = EventSenderImpl<MsgTrait>;


pub struct InnerNetHandler<M: MsgTrait> {
    _node_id: NID,
    sync_service: bool,
    message_async_ch_sender: Mutex<Vec<Arc<MessageChAsyncSender<(Message<M>, Arc<dyn EndpointAsync<M>>)>>>>,
    message_async_ch_receiver: Vec<Arc<MessageReceiverChannelAsync<M>>>,
    message_sync_ch_sender: Mutex<Vec<Arc<MessageChSyncSender<(Message<M>, Arc<dyn EndpointSync<M>>)>>>>,
    message_sync_ch_receiver: Vec<Arc<MessageReceiverChannelSync<M>>>,
    stop_notify: Notifier,
}

#[derive(Clone)]
pub struct NetHandler<M: MsgTrait> {
    name: String,
    inner: Arc<InnerNetHandler<M>>,
}

#[async_trait]
impl<M: MsgTrait> HandleEvent<M> for NetHandler<M> {
    async fn on_accepted(&self, endpoint: Arc<dyn EndpointAsync<M>>) -> Res<()> {
        trace!("{} accept connection {}", self.name, endpoint.remote_address().to_string());
        let inner = self.inner.clone();
        spawn_local_task(inner.stop_notify.clone(), "handle_message, ", async move {
            let _r = inner.process_message(endpoint).await;
        })?;
        Ok(())
    }

    async fn on_connected(
        &self,
        address: SocketAddr,
        result_endpoint: Res<Arc<dyn EndpointAsync<M>>>,
    ) -> Res<()> {
        match result_endpoint {
            Ok(endpoint) => {
                trace!("connect server {}", endpoint.remote_address().to_string());
            }
            Err(e) => {
                trace!("connect server {} fail {}", address.to_string(), e.to_string());
            }
        }

        Ok(())
    }

    async fn on_error(&self, error: ET) {
        self.inner.error(error);
    }


    async fn on_stop(&self) {
        self.inner.stop().await;
    }
}

impl<M: MsgTrait> NetHandler<M> {
    pub fn new(node_id: NID,
               name: String,
               sync_service: bool,
               num_message_receiver: u32,
               stop_notify: Notifier,
    ) -> Self {
        Self {
            name,
            inner: Arc::new(InnerNetHandler::new(
                node_id,
                sync_service,
                num_message_receiver,
                stop_notify)),
        }
    }

    pub fn message_receiver_async(&self) -> Vec<Arc<MessageReceiverChannelAsync<M>>> {
        self.inner.message_receiver_async()
    }

    pub fn message_receiver_sync(&self) -> Vec<Arc<MessageReceiverChannelSync<M>>> {
        self.inner.message_receiver_sync()
    }
}

impl<M: MsgTrait> InnerNetHandler<M> {
    pub fn new(
        node_id: NID,
        sync_service: bool,
        num_message_receiver: u32,
        stop_notify: Notifier,
    ) -> Self {
        if num_message_receiver == 0 {
            panic!("cannot 0 message receiver");
        }
        let mut message_async_ch_sender = vec![];
        let mut message_async_ch_receiver: Vec<Arc<MessageReceiverChannelAsync<M>>> = vec![];

        let mut message_sync_ch_sender = vec![];
        let mut message_sync_ch_receiver: Vec<Arc<MessageReceiverChannelSync<M>>> = vec![];
        if !sync_service {
            for _ in 0..num_message_receiver {
                let (s, r) = mpsc::unbounded_channel::<(Message<M>, Arc<dyn EndpointAsync<M>>)>();
                let receiver = MessageReceiverChannelAsync::new(Arc::new(Mutex::new(r)));
                let r = Arc::new(receiver);
                message_async_ch_receiver.push(r.clone());
                message_async_ch_sender.push(Arc::new(s));
            }
        } else {
            for _ in 0..num_message_receiver {
                let (s, r) = std::sync::mpsc::channel::<(Message<M>, Arc<dyn EndpointSync<M>>)>();
                let receiver = MessageReceiverChannelSync::new(Arc::new(std::sync::Mutex::new(r)));
                let r = Arc::new(receiver);
                message_sync_ch_receiver.push(r.clone());
                message_sync_ch_sender.push(Arc::new(s));
            }
        }

        let s = InnerNetHandler {
            _node_id: node_id,
            sync_service,
            message_async_ch_sender: Mutex::new(message_async_ch_sender),
            message_async_ch_receiver,
            message_sync_ch_sender: Mutex::new(message_sync_ch_sender),
            message_sync_ch_receiver,
            stop_notify,
        };
        s
    }

    fn message_receiver_async(&self) -> Vec<Arc<MessageReceiverChannelAsync<M>>> {
        self.message_async_ch_receiver.clone()
    }

    fn message_receiver_sync(&self) -> Vec<Arc<MessageReceiverChannelSync<M>>> {
        self.message_sync_ch_receiver.clone()
    }

    async fn receiver_message(&self, message: Message<M>, ep: Arc<dyn EndpointAsync<M>>) -> Res<()> {
        let mut hasher = DefaultHasher::new();
        message.hash(&mut hasher);
        let hash = hasher.finish();
        if self.sync_service {
            let guard = self.message_sync_ch_sender.lock().await;
            let index = (hash as usize) % guard.len();
            let n: Arc<MessageChSyncSender<(Message<M>, Arc<dyn EndpointSync<M>>)>> = guard[index].clone();
            let ep_sync = Arc::new(EndpointSyncImpl::new(ep));
            EndpointSyncImpl::handle_loop(ep_sync.clone(), self.stop_notify.clone());
            let result = n.send((message, ep_sync));
            match result {
                Ok(_) => { Ok(()) }
                Err(e) => { Err(ET::TokioSenderError(e.to_string())) }
            }
        } else {
            let guard = self.message_async_ch_sender.lock().await;
            let index = (hash as usize) % guard.len();
            let n: Arc<MessageChAsyncSender<(Message<M>, Arc<dyn EndpointAsync<M>>)>> = guard[index].clone();
            let result = n.send((message, ep));
            match result {
                Ok(_) => { Ok(()) }
                Err(e) => { Err(ET::TokioSenderError(e.to_string())) }
            }
        }

    }


    async fn stop(&self) {
        let mut guard = self.message_async_ch_sender.lock().await;
        guard.clear();
    }


    async fn process_message(
        &self,
        ep: Arc<dyn EndpointAsync<M>>,
    ) -> Res<()> {
        let r = self.loop_handle_message(&ep)
            .instrument(trace_span!("loop handle message")).await;
        match r {
            Ok(()) => {}
            Err(e) => {
                match e {
                    ET::EOF => {
                        trace!("connection eof");
                        return Err(ET::EOF);
                    }
                    _ => {
                        self.error(e);
                    }
                }
            }
        }
        let _ = ep.close().await;
        Ok(())
    }

    async fn loop_handle_message(
        &self,
        ep: &Arc<dyn EndpointAsync<M>>,
    ) -> Res<()> {
        let mut r = Ok(());
        while r.is_ok() {
            r = self.handle_next_message(ep).await;
            match &r {
                Ok(()) => {}
                Err(e) => {
                    match e {
                        ET::EOF => {
                            trace!("connection eof")
                        }
                        _ => { self.error(e.clone()); }
                    }
                    break;
                }
            }
        }
        r
    }

    async fn handle_next_message(
        &self,
        ep: &Arc<dyn EndpointAsync<M>>,
    ) -> Res<()> {
        let m = ep.recv().await?;
        self.receiver_message(m, ep.clone()).await?;
        Ok(())
    }

    fn error(&self, et: ET) {
        error!("error {:?}", et);
    }
}


#[derive(
Clone,
Hash,
PartialEq,
Eq,
Debug,
Serialize,
Deserialize,
Decode,
Encode,
)]
struct TestMsg {}

impl MsgTrait for TestMsg {}

#[test]
#[should_panic(expected = "cannot 0 message receiver")]
fn test_net_handler() {
    let _ = NetHandler::<TestMsg>::new(
        1 as NID,
        "handler".to_string(),
        false,
        0,
        Notifier::default());
}