use std::sync::Arc;

use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use tokio::runtime::Runtime;
use tokio::task::LocalSet;

use crate::event_sink_async::EventSinkAsync;
use crate::message_receiver_async::{ReceiverAsync, ReceiverRRAsync};
use crate::message_sender_async::{SenderAsync, SenderRRAsync};

pub trait IOServiceAsync<M: MsgTrait>: Send + Sync {
    fn local_run(&self, local_set:&LocalSet);

    fn block_run(&self, opt: Option<LocalSet>, runtime: Arc<Runtime>);

    fn node_id(&self) -> NID;

    fn default_sink(&self) -> Arc<dyn EventSinkAsync<M>>;

    fn default_sender(&self) -> Arc<dyn SenderAsync<M>>;

    fn new_sink(&self, name: String) -> Res<Arc<dyn EventSinkAsync<M>>>;

    fn new_sender(&self, name: String) -> Res<Arc<dyn SenderAsync<M>>>;

    fn receiver(&self) -> Vec<Arc<dyn ReceiverAsync<M>>>;

    fn receiver_rr(&self) -> Vec<Arc<dyn ReceiverRRAsync<M>>>;

    fn default_sender_rr(&self) -> Arc<dyn SenderRRAsync<M>>;

    fn new_sender_rr(&self, name: String) -> Res<Arc<dyn SenderRRAsync<M>>>;
}