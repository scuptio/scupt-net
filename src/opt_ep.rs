pub struct OptEP {
    dtm_test: bool,
}


impl OptEP {
    pub fn new() -> Self {
        Self {
            dtm_test: false,
        }
    }


    pub fn is_enable_dtm_test(&self) -> bool { self.dtm_test }


    pub fn enable_dtm_test(self, dtm_test: bool) -> Self {
        let mut s = self;
        s.dtm_test = dtm_test;
        s
    }
}

impl Default for OptEP {
    fn default() -> Self {
        Self::new()
    }
}