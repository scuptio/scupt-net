use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::error_type::ET;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;
use tokio::sync::Mutex;
use crate::endpoint::Endpoint;

use crate::message_channel::MessageChReceiver;
use crate::message_receiver::{Receiver, ReceiverRR};
use crate::message_sender::SenderResp;
use crate::message_sender_endpoint::MessageSenderEndpoint;

pub struct MessageReceiverChannel<M: MsgTrait + 'static> {
    receiver: Arc<Mutex<MessageChReceiver<(Message<M>, Endpoint)>>>,
}

impl<M: MsgTrait + 'static> MessageReceiverChannel<M> {
    pub(crate) fn new(receiver: Arc<Mutex<MessageChReceiver<(Message<M>, Endpoint)>>>) -> Self {
        Self {
            receiver
        }
    }
}

impl<M: MsgTrait + 'static> Clone for MessageReceiverChannel<M> {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
        }
    }
}

#[async_trait]
impl<M: MsgTrait + 'static> Receiver<M> for MessageReceiverChannel<M> {
    async fn receive(&self) -> Res<Message<M>> {
        let mut guard = self.receiver.lock().await;
        let opt = guard.recv().await;
        match opt {
            Some((message, _ep)) => { Ok(message) }
            None => {
                // the sender is closed
                Err(ET::EOF)
            }
        }
    }
}

#[async_trait]
impl<M: MsgTrait + 'static> ReceiverRR<M> for MessageReceiverChannel<M> {
    async fn receive(&self) -> Res<(Message<M>, Arc<dyn SenderResp<M>>)> {
        let mut guard = self.receiver.lock().await;
        let opt = guard.recv().await;
        match opt {
            Some((message, ep)) => {
                Ok((message,
                    Arc::new(MessageSenderEndpoint::new(ep)))) }
            None => {
                // the sender is closed
                Err(ET::EOF)
            }
        }
    }
}