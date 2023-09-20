use async_trait::async_trait;
use scupt_util::message::MsgTrait;
use scupt_util::res::Res;

use crate::message_incoming::MessageIncoming;

pub struct MessageIncomingDummy {}

impl MessageIncomingDummy {
    fn new() -> Self {
        Self {}
    }
}

impl Default for MessageIncomingDummy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<M: MsgTrait + 'static> MessageIncoming<M> for MessageIncomingDummy {
    async fn incoming(&self, _: M) -> Res<()> {
        Ok(())
    }
}