use std::marker::PhantomData;

use async_trait::async_trait;
use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;

use crate::event_sink::ESOption;
use crate::message_sender::MessageSender;
use crate::opt_send::OptSend;

pub type ESSendOpt = ESOption;


pub struct MessageSenderDummy<M: MsgTrait + 'static> {
    _phantom: PhantomData<M>,
}

impl<M> MessageSenderDummy<M>
    where M: MsgTrait + 'static
{
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData::default(),
        }
    }
}

impl<M> Default for MessageSenderDummy<M>
    where M: MsgTrait + 'static
{
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<M> MessageSender<M> for MessageSenderDummy<M>
    where M: MsgTrait + 'static
{
    async fn send(&self, _: NID, _: M, _: OptSend) -> Res<()> {
        Ok(())
    }
}

unsafe impl<M> Sync for MessageSenderDummy<M>
    where M: MsgTrait + 'static
{}

unsafe impl<M> Send for MessageSenderDummy<M>
    where M: MsgTrait + 'static
{}
