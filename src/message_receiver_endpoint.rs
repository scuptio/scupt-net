use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;

use crate::endpoint_async::EndpointAsync;
use crate::message_receiver_async::ReceiverResp;

pub struct MessageReceiverEndpoint<M: MsgTrait + 'static> {
    ep: Arc<dyn EndpointAsync<M>>,
    _pd: PhantomData<M>,
}

impl<M: MsgTrait + 'static> MessageReceiverEndpoint<M> {
    pub fn new(ep: Arc<dyn EndpointAsync<M>>) -> Self {
        Self {
            ep,
            _pd: Default::default(),
        }
    }
}

#[async_trait]
impl<M: MsgTrait + 'static> ReceiverResp<M> for MessageReceiverEndpoint<M> {
    async fn receive(&self) -> Res<Message<M>> {
        self.ep.recv().await
    }
}