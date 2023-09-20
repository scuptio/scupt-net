use async_trait::async_trait;
use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;

use crate::opt_send::OptSend;



#[async_trait]
pub trait MessageSender<
    M: MsgTrait + 'static,
>: Sync + Send {
    async fn send(&self, node_id: NID, message: M, opt: OptSend) -> Res<()>;
}
