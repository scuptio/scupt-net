
pub struct OptSend {
    no_wait: bool
}



impl OptSend {
    pub fn new() -> Self {
        Self {
            no_wait: false
        }
    }

    pub fn is_enable_no_wait(&self) -> bool {
        self.no_wait
    }

    pub fn enable_no_wait(self, no_wait: bool) -> Self {
        let mut s = self;
        s.no_wait = no_wait;
        s
    }
}

impl Default for OptSend {
    fn default() -> Self {
        Self::new()
    }
}