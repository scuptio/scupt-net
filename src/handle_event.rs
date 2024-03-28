use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::error_type::ET;
use scupt_util::message::MsgTrait;
use scupt_util::res::Res;

use crate::endpoint_async::EndpointAsync;

#[async_trait]
pub trait HandleEvent<M: MsgTrait + 'static>: Sync + Send {
    // server sink
    async fn on_accepted(
        &self,
        endpoint: Arc<dyn EndpointAsync<M>>) -> Res<()>;

    // client sink
    async fn on_connected(
        &self,
        address: SocketAddr,
        endpoint: Res<Arc<dyn EndpointAsync<M>>>,
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
impl<M: MsgTrait> HandleEvent<M> for HandleEventDummy {
    async fn on_accepted(&self, _: Arc<dyn EndpointAsync<M>>) -> Res<()> {
        Ok(())
    }

    async fn on_connected(
        &self,
        _: SocketAddr,
        _: Res<Arc<dyn EndpointAsync<M>>>,
    ) -> Res<()> {
        Ok(())
    }

    async fn on_error(&self, _: ET) {}


    async fn on_stop(&self) {}
}