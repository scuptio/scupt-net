use std::net::SocketAddr;

use async_trait::async_trait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;

use crate::endpoint::Endpoint;

pub struct ESOption {
    no_wait: bool,
}

pub struct ESConnectOption {
    no_wait: bool,
    return_endpoint: bool,
}

pub type ESStopOpt = ESOption;
pub type ESServeOpt = ESOption;
pub type ESConnectOpt = ESConnectOption;

pub type ESSignalOpt = ESOption;

#[async_trait]
pub trait EventSink: Sync + Send {
    async fn stop(&self, opt: ESStopOpt) -> Res<()>;

    async fn serve(&self, addr: SocketAddr, opt: ESServeOpt) -> Res<()>;

    async fn connect(&self, node_id: NID, address: SocketAddr, opt: ESConnectOpt) -> Res<Option<Endpoint>>;
}

impl ESOption {
    pub fn new() -> Self {
        Self {
            no_wait: false,
        }
    }

    pub fn no_wait(&self) -> bool {
        self.no_wait
    }


    pub fn enable_no_wait(self, no_wait: bool) -> Self {
        let mut s = self;
        s.no_wait = no_wait;
        s
    }
}

impl ESConnectOption {
    pub fn new() -> Self {
        Self {
            no_wait: false,
            return_endpoint: false,
        }
    }

    pub fn no_wait(&self) -> bool {
        self.no_wait
    }

    pub fn return_endpoint(&self) -> bool {
        self.return_endpoint
    }
    pub fn enable_no_wait(self, no_wait: bool) -> Self {
        let mut s = self;
        s.no_wait = no_wait;
        s
    }

    pub fn enable_return_endpoint(self, return_endpoint: bool) -> Self {
        let mut s = self;
        s.return_endpoint = return_endpoint;
        s
    }
}

impl Default for ESOption {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ESConnectOption {
    fn default() -> Self {
        Self::new()
    }
}

