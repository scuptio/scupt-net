use std::sync::Arc;
use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use tokio::runtime::Runtime;
use tokio::task::LocalSet;
use crate::event_sink_sync::EventSinkSync;
use crate::message_receiver_sync::ReceiverSync;
use crate::message_sender_sync::SenderSync;


pub trait IOServiceSync<M:MsgTrait> : Send + Sync {
    fn block_run(&self, opt: Option<LocalSet>, runtime: Arc<Runtime>);

    fn node_id(&self) -> NID;

    fn default_sink(&self) -> Arc<dyn EventSinkSync<M>>;

    fn default_sender(&self) -> Arc<dyn SenderSync<M>>;

    fn new_sink(&self, name: String) -> Res<Arc<dyn EventSinkSync<M>>>;

    fn new_sender(&self, name: String) -> Res<Arc<dyn SenderSync<M>>>;

    fn receiver(&self) -> Vec<Arc<dyn ReceiverSync<M>>>;
}