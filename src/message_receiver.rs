use async_trait::async_trait;
use scupt_util::message::MsgTrait;
use scupt_util::res::Res;

#[async_trait]
pub trait MessageReceiver<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn receive(&self) -> Res<M>;
}
