use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;

use crate::opt_send::OptSend;

pub trait SenderSync<
    M: MsgTrait + 'static,
>: Sync + Send {
    fn send(&self, message: Message<M>, opt: OptSend) -> Res<()>;
}
