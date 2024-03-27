use std::sync::Arc;
use scupt_util::error_type::ET;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;
use std::sync::Mutex;

use crate::endpoint_sync::EndpointSync;
use crate::message_channel::MessageChSyncReceiver;
use crate::message_receiver_sync::ReceiverSync;

pub struct MessageReceiverChannelSync<M: MsgTrait + 'static> {
    receiver: Arc<Mutex<MessageChSyncReceiver<(Message<M>, Arc<dyn EndpointSync<M>>)>>>,
}

impl<M: MsgTrait + 'static> MessageReceiverChannelSync<M> {
    pub fn new(receiver: Arc<Mutex<MessageChSyncReceiver<(Message<M>, Arc<dyn EndpointSync<M>>)>>>) -> Self {
        Self {
            receiver
        }
    }
}

impl<M: MsgTrait + 'static> Clone for MessageReceiverChannelSync<M> {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
        }
    }
}


impl<M: MsgTrait + 'static> ReceiverSync<M> for MessageReceiverChannelSync<M> {
    fn receive(&self) -> Res<Message<M>> {
        let guard = self.receiver.lock().unwrap();
        let opt = guard.recv();
        match opt {
            Ok((message, _ep)) => { Ok(message) }
            Err(_e) => {
                // the sender is closed
                Err(ET::EOF)
            }
        }
    }
}

