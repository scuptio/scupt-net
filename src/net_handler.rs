use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bincode::{Decode, Encode};
use scupt_util::error_type::ET;
use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, Instrument, trace, trace_span};

use crate::endpoint::Endpoint;
use crate::event_sink_impl::EventSenderImpl;
use crate::handle_event::HandleEvent;
use crate::message_channel::MessageChSender;
use crate::message_receiver::MessageReceiver;
use crate::message_receiver_impl::MessageReceiverImpl;
use crate::notifier::{Notifier, spawn_cancelable_task};

pub type NodeSender<MsgTrait> = EventSenderImpl<MsgTrait>;


pub struct InnerNetHandler<M: MsgTrait> {
    _node_id: NID,
    message_ch_sender: Mutex<Vec<Arc<MessageChSender<M>>>>,
    message_receiver: Vec<Arc<dyn MessageReceiver<M>>>,
    stop_notify: Notifier,
}

#[derive(Clone)]
pub struct NetHandler<M: MsgTrait> {
    name: String,
    inner: Arc<InnerNetHandler<M>>,
}

#[async_trait]
impl<M: MsgTrait> HandleEvent for NetHandler<M> {
    async fn on_accepted(&self, endpoint: Endpoint) -> Res<()> {
        debug!("{} accept connection {}", self.name, endpoint.remote_address().to_string());
        let inner = self.inner.clone();
        spawn_cancelable_task(inner.stop_notify.clone(), "handle_message, ", async move {
            let _ = inner.process_message(endpoint).await;
        });
        Ok(())
    }

    async fn on_connected(
        &self,
        address: SocketAddr,
        result_endpoint: Res<Endpoint>,
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
               num_message_receiver: u32,
               stop_notify: Notifier,
    ) -> Self {
        Self {
            name,
            inner: Arc::new(InnerNetHandler::new(node_id, num_message_receiver, stop_notify)),
        }
    }

    pub fn message_receiver(&self) -> Vec<Arc<dyn MessageReceiver<M>>> {
        self.inner.message_receiver()
    }
}

impl<M: MsgTrait> InnerNetHandler<M> {
    pub fn new(
        node_id: NID,
        num_message_receiver: u32,
        stop_notify: Notifier,
    ) -> Self {
        if num_message_receiver == 0 {
            panic!("cannot 0 message receiver");
        }
        let mut message_ch_sender = vec![];
        let mut message_receiver: Vec<Arc<dyn MessageReceiver<M>>> = vec![];
        for _ in 0..num_message_receiver {
            let (s, r) = mpsc::unbounded_channel::<M>();
            let receiver = MessageReceiverImpl::new(Arc::new(Mutex::new(r)));
            message_receiver.push(Arc::new(receiver));
            message_ch_sender.push(Arc::new(s));
        }

        let s = InnerNetHandler {
            _node_id: node_id,
            message_ch_sender: Mutex::new(message_ch_sender),
            message_receiver,
            stop_notify,
        };
        s
    }

    fn message_receiver(&self) -> Vec<Arc<dyn MessageReceiver<M>>> {
        self.message_receiver.clone()
    }

    async fn receiver_message(&self, message: M) -> Res<()> {
        let mut hasher = DefaultHasher::new();
        message.hash(&mut hasher);
        let hash = hasher.finish();
        let guard = self.message_ch_sender.lock().await;
        let index = (hash as usize) % guard.len();
        let n: Arc<MessageChSender<M>> = guard[index].clone();
        let result = n.send(message);
        match result {
            Ok(_) => { Ok(()) }
            Err(e) => { Err(ET::TokioSenderError(e.to_string())) }
        }
    }


    async fn stop(&self) {
        let mut guard = self.message_ch_sender.lock().await;
        guard.clear();
    }


    async fn process_message(
        &self,
        ep: Endpoint,
    ) -> Res<()> {
        let r = self.loop_handle_message(&ep)
            .instrument(trace_span!("loop handle message")).await;

        match r {
            Ok(()) => {}
            Err(e) => {
                match e {
                    ET::StopService => {
                        trace!("connection eof");
                        return Err(ET::StopService);
                    }
                    _ => { self.error(e); }
                }
            }
        }
        Ok(())
    }

    async fn loop_handle_message(
        &self,
        ep: &Endpoint,
    ) -> Res<()> {
        let mut r = Ok(());
        while r.is_ok() {
            r = self.handle_next_message(ep).await;
            match &r {
                Ok(()) => {}
                Err(e) => {
                    match e {
                        ET::StopService => {
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
        ep: &Endpoint,
    ) -> Res<()> {
        let m = ep.recv::<M>().await?;
        self.receiver_message(m).await?;
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
        0,
        Notifier::default());
}