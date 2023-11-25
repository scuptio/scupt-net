use std::marker::PhantomData;
use async_trait::async_trait;
use scupt_util::message::MsgTrait;
use scupt_util::res::Res;
use crate::endpoint::Endpoint;
use crate::message_receiver::MessageReceiver;

pub struct  MessageReceiverEndpoint<M:MsgTrait + 'static> {
    ep : Endpoint,
    _pd : PhantomData<M>
}

impl <M:MsgTrait + 'static> MessageReceiverEndpoint<M> {
    pub fn new(ep:Endpoint) -> Self {
        Self {
            ep,
            _pd: Default::default(),
        }
    }
}

#[async_trait]
impl <M:MsgTrait + 'static> MessageReceiver<M> for MessageReceiverEndpoint<M> {
    async fn receive(&self) -> Res<M> {
        self.ep.recv().await
    }
}