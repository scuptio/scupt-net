use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::error_type::ET;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;
use tokio::sync::Mutex;

use crate::endpoint_async::EndpointAsync;
use crate::message_channel::MessageChAsyncReceiver;
use crate::message_receiver_async::{ReceiverAsync, ReceiverRRAsync};
use crate::message_sender_async::SenderRespAsync;
use crate::message_sender_endpoint::MessageSenderEndpoint;

pub struct MessageReceiverChannelAsync<M: MsgTrait + 'static> {
    receiver: Arc<Mutex<MessageChAsyncReceiver<(Message<M>, Arc<dyn EndpointAsync<M>>)>>>,
}

impl<M: MsgTrait + 'static> MessageReceiverChannelAsync<M> {
    pub fn new(receiver: Arc<Mutex<MessageChAsyncReceiver<(Message<M>, Arc<dyn EndpointAsync<M>>)>>>) -> Self {
        Self {
            receiver
        }
    }
}

impl<M: MsgTrait + 'static> Clone for MessageReceiverChannelAsync<M> {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
        }
    }
}

#[async_trait]
impl<M: MsgTrait + 'static> ReceiverAsync<M> for MessageReceiverChannelAsync<M> {
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
impl<M: MsgTrait + 'static> ReceiverRRAsync<M> for MessageReceiverChannelAsync<M> {
    async fn receive(&self) -> Res<(Message<M>, Arc<dyn SenderRespAsync<M>>)> {
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