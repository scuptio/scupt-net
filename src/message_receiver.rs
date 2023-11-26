use std::sync::Arc;
use async_trait::async_trait;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;
use crate::message_sender::SenderResp;

#[async_trait]
pub trait Receiver<
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
pub trait ReceiverRR<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn receive(&self) -> Res<(Message<M>, Arc<dyn SenderResp<M>>)>;
}