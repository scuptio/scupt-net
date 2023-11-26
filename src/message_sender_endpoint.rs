use std::marker::PhantomData;

use async_trait::async_trait;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;
use crate::endpoint::Endpoint;
use crate::message_sender::SenderResp;
pub struct  MessageSenderEndpoint<M:MsgTrait + 'static> {
    ep : Endpoint,
    _pd : PhantomData<M>
}

impl <M:MsgTrait + 'static> MessageSenderEndpoint<M> {
    pub fn new(ep:Endpoint) -> Self {
        Self {
            ep,
            _pd: Default::default(),
        }
    }
}

#[async_trait]
impl <M:MsgTrait + 'static> SenderResp<M> for MessageSenderEndpoint<M> {
    async fn send(&self, m: Message<M>) -> Res<()> {
        self.ep.send(m).await?;
        Ok(())
    }
}