use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use scupt_util::res::Res;
use scupt_util::message::{Message, MsgTrait};
use async_trait::async_trait;
use scupt_util::error_type::ET;
use scupt_util::node_id::NID;
use tokio::task::LocalSet;
use tokio::time::sleep;


use crate::endpoint::Endpoint;
use crate::event_sink::ESConnectOption;
use crate::handle_event::HandleEvent;
use crate::node::Node;
use crate::notifier::Notifier;

#[derive(Clone)]
pub struct Client<M:MsgTrait + 'static> {
    inner: Arc<ClientInner<M>>,
}
pub struct ClientInner<M:MsgTrait +'static> {
    nid:NID,
    addr:String,
    node:Node<M, Handler>,
    opt_endpoint: Mutex<Option<Endpoint>>,
}


struct Handler {

}

impl <M:MsgTrait +'static> Client<M> {
    pub fn new(node_id:NID, name:String, addr:String, notifier:Notifier) -> Res<Self> {
        Ok(Self {
            inner:Arc::new(ClientInner::new(node_id, name, addr, notifier)?)
        })
    }

    pub fn run(&self, local:&LocalSet) {
        self.inner.run(local);
    }

    pub async fn connect(&self, opt:OptClientConnect) -> Res<()> {
        self.inner.connect(opt).await
    }

    pub async fn send(&self, message:Message<M>) -> Res<()> {
        self.inner.send(message).await
    }

    pub async fn recv(&self) -> Res<Message<M>> {
        self.inner.recv().await
    }

    pub fn node_id(&self) -> NID {
        self.inner.nid
    }

    pub fn server_addr(&self) -> String {
        self.inner.addr.clone()
    }
}

impl Handler {
    fn new() -> Self {
        Self {}
    }
}

pub struct OptClientConnect {
    pub retry_max:u64,
    pub retry_wait_ms:u64
}

impl OptClientConnect {
    pub fn new() -> Self {
        Self {
            retry_max: 0,
            retry_wait_ms: 50,
        }
    }
}

impl Default for OptClientConnect {
    fn default() -> Self {
        Self::new()
    }
}

impl <M:MsgTrait +'static> ClientInner<M> {
    pub fn new(node_id:NID, name:String, addr:String, notifier: Notifier) -> Res<Self> {
        let r = Self {
            nid: node_id.clone(),
            addr,
            node: Node::new(node_id, name, Handler::new(), notifier)?,
            opt_endpoint: Default::default(),
        };
        Ok(r)
    }

    pub fn run(&self, local:&LocalSet) {
        self.node.run_local(local);
    }

    pub async fn connect(&self, opt:OptClientConnect) -> Res<()> {
        let mut opt_ep = None;
        let mut n = opt.retry_max;
        while opt.retry_max == 0 || n > 0 {
            let sockaddr = SocketAddr::from_str(self.addr.as_str()).unwrap();
            let r = self.node.default_event_sink().connect(
                self.nid, sockaddr,
                ESConnectOption::new()
                    .enable_no_wait(false)
                    .enable_return_endpoint(true)).await;
            if let Ok(e) = r {
                opt_ep = e;
                break;
            } else {
                sleep(Duration::from_millis(opt.retry_wait_ms)).await;
            }
            if n > 0 {
                n -= 1;
            }
        };

        if let Some(e) = opt_ep {
            let mut guard = self.opt_endpoint.lock().await;
            *guard = Some(e);
        }
        Ok(())
    }

    pub async fn send(&self, message:Message<M>) -> Res<()> {
        let guard = self.opt_endpoint.lock().await;
        if let Some(e) = &(*guard) {
            e.send(message).await?;
            return Ok(())
        } else {
            Err(ET::NetNotConnected)
        }
    }

    pub async fn recv(&self) -> Res<Message<M>> {
        let guard = self.opt_endpoint.lock().await;
        if let Some(e) = &(*guard) {
            let m = e.recv().await?;
            return Ok(m)
        } else {
            Err(ET::NetNotConnected)
        }
    }
}

#[async_trait]
impl HandleEvent for Handler {
    async fn on_accepted(&self, _: Endpoint) -> Res<()> {
        Ok(())
    }

    async fn on_connected(&self, _: SocketAddr, _: Res<Endpoint>) -> Res<()> {
        Ok(())
    }

    async fn on_error(&self, _: ET) {

    }

    async fn on_stop(&self) {

    }
}