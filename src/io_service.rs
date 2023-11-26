use std::sync::Arc;

use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use tokio::runtime::Runtime;
use tokio::task::LocalSet;

use crate::event_sink::EventSink;
use crate::message_receiver::{Receiver, ReceiverRR};
use crate::message_receiver_channel::MessageReceiverChannel;
use crate::message_sender::{Sender, SenderRR};
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
    receiver: Vec<Arc<MessageReceiverChannel<M>>>
}


pub struct IOServiceOpt {
    pub num_message_receiver: u32
}

impl<M: MsgTrait> IOService<M> {
    pub fn new(node_id: NID,
               name: String,
               opt: IOServiceOpt,
               stop_notify: Notifier,
    ) -> Res<Self> {
        let handler = NetHandler::<M>::new(node_id.clone(),
                                           name.clone(),
                                           opt.num_message_receiver,
                                           stop_notify.clone());
        let receivers = handler.message_receiver();
        let node = ServiceNode::new(
            node_id.clone(),
            name,
            handler.clone(),
            stop_notify)?;

        let s = Self {
            node_id,
            node,
            receiver: receivers
        };
        Ok(s)
    }

    pub fn node_id(&self) -> NID {
        self.node_id.clone()
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

    pub fn new_event_sender(&self, name: String) -> Res<Arc<dyn EventSink>> {
        self.node.new_event_channel(name)
    }


    pub fn default_event_sink(&self) -> Arc<dyn EventSink> {
        self.node.default_event_sink()
    }

    pub fn default_message_sender(&self) -> Arc<dyn Sender<M>> {
        self.node.default_message_sender()
    }

    pub fn new_message_sender(&self, name: String) -> Res<Arc<dyn Sender<M>>> {
        self.node.new_message_sender(name)
    }


    pub fn default_message_sender_rr(&self) -> Arc<dyn SenderRR<M>> {
        self.node.default_message_sender_rr()
    }

    pub fn new_message_sender_rr(&self, name: String) -> Res<Arc<dyn SenderRR<M>>> {
        self.node.new_message_sender_rr(name)
    }

    pub fn message_receiver(&self) -> Vec<Arc<dyn Receiver<M>>> {
        if self.receiver.is_empty() {
            panic!("todo");
        }
        self.receiver.iter().map(|r| {
            let m : Arc<dyn Receiver<M>> = r.clone();
            m
        }).collect()
    }

    pub fn message_receiver_rr(&self) -> Vec<Arc<dyn ReceiverRR<M>>> {
        if self.receiver.is_empty() {
            panic!("todo");
        }
        self.receiver.iter().map(|r| {
            let m : Arc<dyn ReceiverRR<M>> = r.clone();
            m
        }).collect()
    }
}


