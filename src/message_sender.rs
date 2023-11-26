use async_trait::async_trait;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;
use crate::message_receiver::ReceiverOneshot;
use crate::opt_send::OptSend;

#[async_trait]
pub trait Sender<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn send(&self, message: Message<M>, opt: OptSend) -> Res<()>;
}

#[async_trait]
pub trait SenderRR<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn send(&self, message: Message<M>, opt: OptSend) -> Res<Box<dyn ReceiverOneshot<M>>>;
}

#[async_trait]
pub trait SenderOneshot<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn send(self, message: Message<M>) -> Res<()>;
}
