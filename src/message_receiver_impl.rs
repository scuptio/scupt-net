use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::error_type::ET;
use scupt_util::message::MsgTrait;
use scupt_util::res::Res;
use tokio::sync::Mutex;

use crate::message_channel::MessageChReceiver;
use crate::message_receiver::MessageReceiver;

pub struct MessageReceiverImpl<M: MsgTrait + 'static> {
    receiver: Arc<Mutex<MessageChReceiver<M>>>,
}

impl<M: MsgTrait + 'static> MessageReceiverImpl<M> {
    pub(crate) fn new(receiver: Arc<Mutex<MessageChReceiver<M>>>) -> Self {
        Self {
            receiver
        }
    }
}

impl<M: MsgTrait + 'static> Clone for MessageReceiverImpl<M> {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
        }
    }
}

#[async_trait]
impl<M: MsgTrait + 'static> MessageReceiver<M> for MessageReceiverImpl<M> {
    async fn receive(&self) -> Res<M> {
        let mut guard = self.receiver.lock().await;
        let opt = guard.recv().await;
        match opt {
            Some(message) => { Ok(message) }
            None => {
                // the sender is closed
                Err(ET::EOF)
            }
        }
    }
}