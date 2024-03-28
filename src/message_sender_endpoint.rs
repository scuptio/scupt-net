use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;

use crate::endpoint_async::EndpointAsync;
use crate::message_sender_async::SenderRespAsync;

pub struct MessageSenderEndpoint<M: MsgTrait + 'static> {
    ep: Arc<dyn EndpointAsync<M>>,
    _pd: PhantomData<M>,
}

impl<M: MsgTrait + 'static> MessageSenderEndpoint<M> {
    pub fn new(ep: Arc<dyn EndpointAsync<M>>) -> Self {
        Self {
            ep,
            _pd: Default::default(),
        }
    }
}

#[async_trait]
impl<M: MsgTrait + 'static> SenderRespAsync<M> for MessageSenderEndpoint<M> {
    async fn send(&self, m: Message<M>) -> Res<()> {
        self.ep.send(m).await?;
        Ok(())
    }
}