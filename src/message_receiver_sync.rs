use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;

pub trait ReceiverSync<
    M: MsgTrait + 'static,
>: Sync + Send {
    fn receive(&self) -> Res<Message<M>>;
}
