use std::net::SocketAddr;

use async_trait::async_trait;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;

#[async_trait]
pub trait EndpointAsync<M: MsgTrait + 'static>: Send + Sync {
    fn remote_address(&self) -> SocketAddr;

    async fn send(&self, m: Message<M>) -> Res<()>;

    async fn recv(&self) -> Res<Message<M>>;

    async fn close(&self) -> Res<()>;
}
