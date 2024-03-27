use std::sync::Arc;

use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use tokio::runtime::Runtime;
use tokio::task::LocalSet;

use crate::event_sink_async::EventSinkAsync;
use crate::event_sink_sync::EventSinkSync;
use crate::io_service_async::IOServiceAsync;
use crate::io_service_sync::IOServiceSync;
use crate::message_receiver_async::{ReceiverAsync, ReceiverRRAsync};
use crate::message_receiver_channel_async::MessageReceiverChannelAsync;
use crate::message_receiver_channel_sync::MessageReceiverChannelSync;
use crate::message_receiver_sync::ReceiverSync;
use crate::message_sender_async::{SenderAsync, SenderRRAsync};
use crate::message_sender_sync::SenderSync;
use crate::net_handler::NetHandler;
use crate::node::Node;
use crate::notifier::Notifier;

type ServiceNode<M> = Node<
    M,
    NetHandler<M>
>;

pub struct IOService<M: MsgTrait> {
    node_id: NID,
    node: ServiceNode<M>,
    receiver_async: Vec<Arc<MessageReceiverChannelAsync<M>>>,
    receiver_sync: Vec<Arc<MessageReceiverChannelSync<M>>>
}


pub struct IOServiceOpt {
    pub num_message_receiver: u32,
    pub testing: bool,
    pub sync_service: bool,
}

impl<M: MsgTrait> IOService<M> {
    pub fn new_async_service(
        node_id: NID,
        name: String,
        opt: IOServiceOpt,
        stop_notify: Notifier,
    ) -> Res<Arc<dyn IOServiceAsync<M>>> {
        let ios = Self::new(
            node_id,
            name,
            opt,
            stop_notify,
        )?;
        Ok(Arc::new(ios))
    }

    pub fn new_sync_service(
        node_id: NID,
        name: String,
        opt: IOServiceOpt,
        stop_notify: Notifier,
    ) -> Res<Arc<dyn IOServiceSync<M>>> {
        let ios = Self::new(
            node_id,
            name,
            opt,
            stop_notify,
        )?;
        Ok(Arc::new(ios))
    }

    fn new(node_id: NID,
               name: String,
               opt: IOServiceOpt,
               stop_notify: Notifier,
    ) -> Res<Self> {
        let handler = NetHandler::<M>::new(node_id.clone(),
                                           name.clone(),
                                           opt.sync_service,
                                           opt.num_message_receiver,
                                           stop_notify.clone());
        let receiver_async = handler.message_receiver_async();
        let receiver_sync = handler.message_receiver_sync();
        let node = ServiceNode::new(
            node_id.clone(),
            name,
            handler.clone(),
            opt.testing,
            stop_notify)?;

        let s = Self {
            node_id,
            node,
            receiver_async,
            receiver_sync,
        };
        Ok(s)
    }

    pub fn run(&self, opt_ls: Option<LocalSet>, runtime: Arc<Runtime>) {
        let ls = match opt_ls {
            Some(ls) => { ls }
            None => { LocalSet::new() }
        };
        self.node.block_run(Some(ls), runtime);
    }

    pub fn run_local(&self, local_set: &LocalSet) {
        self.node.run_local(local_set);
    }

    fn receiver_async(&self) -> Vec<Arc<dyn ReceiverAsync<M>>> {
        if self.receiver_async.is_empty() {
            panic!("todo");
        }
        self.receiver_async.iter().map(|r| {
            let m: Arc<dyn ReceiverAsync<M>> = r.clone();
            m
        }).collect()
    }

    fn receiver_sync(&self) -> Vec<Arc<dyn ReceiverSync<M>>> {
        if self.receiver_sync.is_empty() {
            panic!("todo");
        }
        self.receiver_sync.iter().map(|r| {
            let m: Arc<dyn ReceiverSync<M>> = r.clone();
            m
        }).collect()
    }

    fn new_sink_async(&self, name: String) -> Res<Arc<dyn EventSinkAsync<M>>> {
        self.node.new_event_channel(name)
    }

    fn new_sink_sync(&self, name: String) -> Res<Arc<dyn EventSinkSync<M>>> {
        self.node.new_event_channel_sync(name)
    }

    fn default_sink_async(&self) -> Arc<dyn EventSinkAsync<M>> {
        self.node.default_event_sink()
    }
    fn default_sink_sync(&self) -> Arc<dyn EventSinkSync<M>> {
        self.node.default_event_sink_sync()
    }
    fn default_sender_async(&self) -> Arc<dyn SenderAsync<M>> {
        self.node.default_message_sender_async()
    }

    fn default_sender_sync(&self) -> Arc<dyn SenderSync<M>> {
        self.node.default_message_sender_sync()
    }
    fn new_sender_async(&self, name: String) -> Res<Arc<dyn SenderAsync<M>>> {
        self.node.new_message_sender_async(name)
    }

    fn new_sender_sync(&self, name: String) -> Res<Arc<dyn SenderSync<M>>> {
        self.node.new_message_sender_sync(name)
    }
    fn default_sender_rr_async(&self) -> Arc<dyn SenderRRAsync<M>> {
        self.node.default_message_sender_rr()
    }

    fn new_sender_rr_async(&self, name: String) -> Res<Arc<dyn SenderRRAsync<M>>> {
        self.node.new_message_sender_rr(name)
    }

    fn receiver_async_rr(&self) -> Vec<Arc<dyn ReceiverRRAsync<M>>> {
        if self.receiver_async.is_empty() {
            panic!("todo");
        }
        self.receiver_async.iter().map(|r| {
            let m : Arc<dyn ReceiverRRAsync<M>> = r.clone();
            m
        }).collect()
    }
}


impl<M: MsgTrait> IOServiceAsync<M> for IOService<M> {
    fn block_run(&self, opt: Option<LocalSet>, runtime: Arc<Runtime>) {
        self.run(opt, runtime);
    }

    fn node_id(&self) -> NID {
        self.node_id
    }

    fn default_sink(&self) -> Arc<dyn EventSinkAsync<M>> {
        self.default_sink_async()
    }

    fn default_sender(&self) -> Arc<dyn SenderAsync<M>> {
        self.default_sender_async()
    }

    fn new_sink(&self, name: String) -> Res<Arc<dyn EventSinkAsync<M>>> {
        self.new_sink_async(name)
    }

    fn new_sender(&self, name: String) -> Res<Arc<dyn SenderAsync<M>>> {
        self.new_sender_async(name)
    }

    fn receiver(&self) -> Vec<Arc<dyn ReceiverAsync<M>>> {
        self.receiver_async()
    }

    fn receiver_rr(&self) -> Vec<Arc<dyn ReceiverRRAsync<M>>> {
        self.receiver_async_rr()
    }

    fn default_sender_rr(&self) -> Arc<dyn SenderRRAsync<M>> {
        self.default_sender_rr_async()
    }

    fn new_sender_rr(&self, name:String) -> Res<Arc<dyn SenderRRAsync<M>>> {
        self.new_sender_rr_async(name)
    }
}


impl<M: MsgTrait> IOServiceSync<M> for IOService<M> {
    fn block_run(&self, opt: Option<LocalSet>, runtime: Arc<Runtime>) {
        self.run(opt, runtime);
    }

    fn node_id(&self) -> NID {
        self.node_id
    }

    fn default_sink(&self) -> Arc<dyn EventSinkSync<M>> {
        self.default_sink_sync()
    }

    fn default_sender(&self) -> Arc<dyn SenderSync<M>> {
        self.default_sender_sync()
    }

    fn new_sink(&self, name: String) -> Res<Arc<dyn EventSinkSync<M>>> {
        self.new_sink_sync(name)
    }

    fn new_sender(&self, name: String) -> Res<Arc<dyn SenderSync<M>>> {
        self.new_sender_sync(name)
    }

    fn receiver(&self) -> Vec<Arc<dyn ReceiverSync<M>>> {
        self.receiver_sync()
    }
}