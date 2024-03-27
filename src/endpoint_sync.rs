use std::net::SocketAddr;

use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;

pub trait EndpointSync<M: MsgTrait + 'static>: Send + Sync {
    fn remote_address(&self) -> SocketAddr;

    fn send(&self, m: Message<M>) -> Res<()>;

    fn recv(&self) -> Res<Message<M>>;

    fn close(&self) -> Res<()>;
}
