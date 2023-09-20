use std::net::SocketAddr;

use async_trait::async_trait;
use scupt_util::error_type::ET;
use scupt_util::res::Res;

use crate::endpoint::Endpoint;

#[async_trait]
pub trait HandleEvent: Sync + Send {
    // server sink
    async fn on_accepted(
        &self,
        endpoint: Endpoint) -> Res<()>;

    // client sink
    async fn on_connected(
        &self,
        address: SocketAddr,
        endpoint: Res<Endpoint>,
    ) -> Res<()>;

    // error sink
    async fn on_error(&self, error: ET);

    // when the runtime stop
    async fn on_stop(&self);
}


#[derive(Clone)]
pub struct HandleEventDummy {}

impl Default for HandleEventDummy {
    fn default() -> Self {
        Self {}
    }
}

#[async_trait]
impl HandleEvent for HandleEventDummy {
    async fn on_accepted(&self, _: Endpoint) -> Res<()> {
        Ok(())
    }

    async fn on_connected(
        &self,
        _: SocketAddr,
        _: Res<Endpoint>,
    ) -> Res<()> {
        Ok(())
    }

    async fn on_error(&self, _: ET) {}


    async fn on_stop(&self) {}
}