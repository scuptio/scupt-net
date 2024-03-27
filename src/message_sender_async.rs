use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;

use crate::message_receiver_async::ReceiverResp;
use crate::opt_send::OptSend;

#[async_trait]
pub trait SenderAsync<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn send(&self, message: Message<M>, opt: OptSend) -> Res<()>;
}

#[async_trait]
pub trait SenderRRAsync<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn send(&self, message: Message<M>, opt: OptSend) -> Res<Arc<dyn ReceiverResp<M>>>;
}

#[async_trait]
pub trait SenderRespAsync<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn send(&self, message: Message<M>) -> Res<()>;
}
