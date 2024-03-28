pub struct ESOption {
    no_wait: bool,
}

pub type ESStopOpt = ESOption;
pub type ESServeOpt = ESOption;
pub type ESConnectOpt = ESConnectOption;

pub type ESSignalOpt = ESOption;

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

pub struct ESConnectOption {
    no_wait: bool,
    return_endpoint: bool,
}

impl Default for ESConnectOption {
    fn default() -> Self {
        Self::new()
    }
}
