use std::collections::HashMap;
use std::sync::Arc;

use rand::seq::SliceRandom;
use rand::thread_rng;
use scupt_util::error_type::ET;
use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use tokio::sync::{mpsc, Mutex};
use tracing::{Instrument, trace, trace_span};

use crate::endpoint_async::EndpointAsync;
use crate::event::{NetEvent, ResultSenderType};
use crate::event_channel::EventChannel;
use crate::net_handler::NodeSender;
use crate::notifier::Notifier;
use crate::task_trace;

pub type EventChannelMap<MsgTrait> = HashMap<String, Arc<EventChannel<MsgTrait>>>;

type SyncMutex<T> = std::sync::Mutex<T>;

struct _NodeContext<M: MsgTrait + 'static> {
    name: String,
    // NodeId to endpoint map
    out_connection_async: HashMap<NID, Vec<Arc<dyn EndpointAsync<M>>>>,
}


pub struct NodeContext<M: MsgTrait + 'static> {
    node_name: String,
    node_id: NID,
    // notifier for stop event
    stop_notify: Notifier,
    mutex_ctx: Mutex<_NodeContext<M>>,
    channel_set: Arc<SyncMutex<EventChannelMap<M>>>,
    default_channel: Arc<EventChannel<M>>,
    enable_testing: bool,
}


impl<M: MsgTrait + 'static> NodeContext<M> {
    pub fn new(node_id: NID, name: String, testing: bool, stop_notify: Notifier) -> Self {
        let mut map = HashMap::new();
        let channel_name = format!("{}_default", name);
        let default_channel = Arc::new(Self::create_event_channel(channel_name));
        map.insert(default_channel.name().clone(), default_channel.clone());
        Self {
            node_name: name.clone(),
            node_id,

            stop_notify,
            mutex_ctx: Mutex::new(_NodeContext::new(name)),
            channel_set: Arc::new(SyncMutex::new(map)),
            default_channel,
            enable_testing: testing,
        }
    }

    pub fn node_id(&self) -> NID {
        self.node_id
    }

    pub fn name(&self) -> &String {
        &self.node_name
    }
    pub fn stop_notify(&self) -> Notifier {
        self.stop_notify.clone()
    }

    #[async_backtrace::framed]
    pub async fn stop_and_notify(&self) {
        let _t = task_trace!();
        let ok = self.stop_notify.task_notify_all();
        if ok {
            self.stop().instrument(trace_span!("stop {}", self.node_id)).await;
        }
    }

    #[async_backtrace::framed]
    pub async fn get_endpoint(&self, node_id: NID) -> Res<Arc<dyn EndpointAsync<M>>> {
        let _t = task_trace!();
        let c = self.mutex_ctx.lock().await;
        c.get_endpoint(node_id)
    }

    #[async_backtrace::framed]
    pub async fn add_endpoint(&self, node_id: NID, endpoint: Arc<dyn EndpointAsync<M>>) -> Res<()> {
        let _t = task_trace!();
        let mut c = self.mutex_ctx.lock().await;
        c.add_endpoint(node_id, endpoint)
    }


    pub fn new_event_channel(&self, name: String) -> Res<Arc<NodeSender<M>>> {
        let (n, s) = self.new_event_sender(name)?;
        Ok(Arc::new(NodeSender::new(n, s)))
    }

    pub fn new_message_sender(&self, name: String) -> Res<Arc<NodeSender<M>>> {
        let (n, s) = self.new_event_sender(name)?;
        Ok(Arc::new(NodeSender::new(n, s)))
    }

    pub fn new_event_sender(&self, name: String) -> Res<(String, mpsc::UnboundedSender<NetEvent<M>>)> {
        let ch = Self::create_event_channel(format!("{}_{}", self.node_id, name));
        let mut map = self.channel_set.lock().unwrap();
        if !map.contains_key(ch.name()) {
            let c = Arc::new(ch);
            map.insert(c.name().clone(), c.clone());
            let r = self.default_channel.sender().send(NetEvent::NewEventChannel(c.clone()));
            match r {
                Ok(_) => {}
                Err(e) => { return Err(ET::SenderError(e.to_string())); }
            };
            Ok((c.name().clone(), c.sender().clone()))
        } else {
            return Err(ET::ExistingSuchElement);
        }
    }

    pub fn default_event_channel(&self) -> Arc<EventChannel<M>> {
        self.default_channel.clone()
    }

    pub fn enable_testing(&self) -> bool {
        self.enable_testing
    }

    #[async_backtrace::framed]
    pub async fn stop(&self) {
        let _t = task_trace!();
        let mut map = self.channel_set.lock().unwrap();
        for (_, v) in map.iter() {
            self.close_one_channel(v.clone()).instrument(trace_span!("close channel ")).await;
        }
        map.clear();
        trace!("stopped {}", self.node_id);
    }

    async fn close_one_channel(&self, ch: Arc<EventChannel<M>>) {
        let result = ch.sender().send(NetEvent::Stop(ResultSenderType::SendNone));
        match result {
            Ok(()) => {}
            Err(e) => { trace!("{}", e.to_string()); }
        }
        ch.sender().closed().await;
        trace!("close one, {}", ch.name());
    }

    fn create_event_channel(name: String) -> EventChannel<M> {
        let channel = EventChannel::new(name);
        channel
    }
}

impl<M: MsgTrait + 'static> _NodeContext<M> {
    pub fn new(name: String) -> Self {
        Self {
            name,
            out_connection_async: Default::default(),
        }
    }

    pub fn get_endpoint(&self, node_id: NID) -> Res<Arc<dyn EndpointAsync<M>>> {
        trace!("get endpoint: {}", self.name);
        match self.out_connection_async.get(&node_id) {
            Some(vec) => {
                let opt = vec.choose(&mut thread_rng());
                match opt {
                    Some(e) => { return Ok(e.clone()); }
                    None => {}
                }
            }
            _ => {}
        }

        Err(ET::NoSuchElement)
    }


    pub fn add_endpoint(&mut self, node_id: NID, endpoint: Arc<dyn EndpointAsync<M>>) -> Res<()> {
        trace!("add endpoint: {}", self.name);
        match self.out_connection_async.get_mut(&node_id) {
            Some(vec) => {
                vec.push(endpoint);
            }
            None => {
                self.out_connection_async.insert(node_id, vec![endpoint]);
            }
        }
        Ok(())
    }
}