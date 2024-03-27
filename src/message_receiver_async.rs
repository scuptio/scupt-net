use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;

use crate::message_sender_async::SenderRespAsync;

#[async_trait]
pub trait ReceiverAsync<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn receive(&self) -> Res<Message<M>>;
}


#[async_trait]
pub trait ReceiverResp<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn receive(&self) -> Res<Message<M>>;
}


#[async_trait]
pub trait ReceiverRRAsync<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn receive(&self) -> Res<(Message<M>, Arc<dyn SenderRespAsync<M>>)>;
}