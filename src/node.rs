use std::net::SocketAddr;
use std::sync::{Arc, Once};

use scupt_util::error_type::ET;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use scupt_util::res_of::res_io;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::task::LocalSet;
use tracing::{error, Instrument, trace, trace_span};

use crate::endpoint_async::EndpointAsync;
use crate::endpoint_async_impl::EndpointAsyncImpl;
use crate::endpoint_sync::EndpointSync;
use crate::endpoint_sync_impl::EndpointSyncImpl;
use crate::event::{NetEvent, ResultSenderType};
use crate::event_channel::EventReceiver;
use crate::event_sink_async::EventSinkAsync;
use crate::event_sink_sync::EventSinkSync;
use crate::handle_event::HandleEvent;
use crate::message_sender_async::{SenderAsync, SenderRRAsync};
use crate::message_sender_sync::SenderSync;
use crate::net_handler::NodeSender;
use crate::node_context::NodeContext;
use crate::notifier::Notifier;
use crate::opt_ep::OptEP;
use crate::task::spawn_local_task;

#[derive(Clone)]
pub struct Node<
    M: MsgTrait + 'static,
    H: HandleEvent<M> + 'static
> {
    _node_id: NID,
    handle: Arc<H>,
    node_context: Arc<NodeContext<M>>,
    run_once: Arc<Once>,
}


impl<
    M: MsgTrait + 'static,
    H: HandleEvent<M> + 'static
>
Node<
    M,
    H
> {
    pub fn new(
        node_id: NID,
        name: String,
        handle: H,
        testing:bool,
        stop_notify: Notifier,
    ) -> Res<Self> {
        let node_context = NodeContext::new(node_id.clone(), name, testing, stop_notify);
        let node = Self {
            _node_id: node_id,
            handle: Arc::new(handle),
            node_context: Arc::new(node_context),
            run_once: Arc::new(Once::new()),
        };
        Ok(node)
    }

    pub fn block_run(&self, opt_ls: Option<LocalSet>, runtime: Arc<Runtime>) {
        let local_set = match opt_ls {
            Some(ls) => { ls }
            None => { LocalSet::new() }
        };
        self.run_local(&local_set);
        runtime.block_on(async move {
            local_set.await;
            trace!("stop run");
        });
    }

    pub fn new_event_channel(&self, name: String) -> Res<Arc<dyn EventSinkAsync<M>>> {
        let r = self.node_context.new_event_channel(name)?;
        Ok(r)
    }

    pub fn new_event_channel_sync(&self, name: String) -> Res<Arc<dyn EventSinkSync<M>>> {
        let r = self.node_context.new_event_channel(name)?;
        Ok(r)
    }
    pub fn new_message_sender_async(&self, name: String) -> Res<Arc<dyn SenderAsync<M>>> {
        let s = self.node_context.new_message_sender(name)?;
        Ok(s)
    }
    pub fn new_message_sender_sync(&self, name: String) -> Res<Arc<dyn SenderSync<M>>> {
        let s = self.node_context.new_message_sender(name)?;
        Ok(s)
    }

    pub fn new_message_sender_rr(&self, name: String) -> Res<Arc<dyn SenderRRAsync<M>>> {
        let s = self.node_context.new_message_sender(name)?;
        Ok(s)
    }

    pub fn stop_notify(&self) -> Notifier {
        self.node_context.stop_notify()
    }

    pub fn default_event_sink(&self) -> Arc<dyn EventSinkAsync<M>> {
        Arc::new(self.node_event_sink())
    }
    pub fn default_event_sink_sync(&self) -> Arc<dyn EventSinkSync<M>> {
        Arc::new(self.node_event_sink())
    }
    pub fn default_message_sender_async(&self) -> Arc<dyn SenderAsync<M>> {
        Arc::new(self.node_event_sink())
    }

    pub fn default_message_sender_sync(&self) -> Arc<dyn SenderSync<M>> {
        Arc::new(self.node_event_sink())
    }

    pub fn default_message_sender_rr(&self) -> Arc<dyn SenderRRAsync<M>> {
        Arc::new(self.node_event_sink())
    }

    pub fn run_local(&self, local_set: &LocalSet) {
        trace!("run local {}", self._node_id);
        self.run_once.call_once(|| {
            self.run_local_once(local_set)
        });
    }

    pub fn run_local_once(&self, local_set: &LocalSet) {
        let name = self.node_context.default_event_channel().name().clone();
        trace!("run local once {}", name);
        let h = self.handle.clone();
        let n = self.node_context.clone();
        let enable_testing = n.enable_testing();
        let c = self.node_context.default_event_channel().receiver().unwrap();
        trace!("main loop {}", n.name());
        let task_name = format!("{}_main_loop", n.name());
        let notify = n.stop_notify();
        let f = async move {
            Self::run_main_loop(
                name,
                n,
                c,
                h,
                enable_testing
            ).instrument(trace_span!("main loop")).await;
        };

        local_set.spawn_local(async move {
            spawn_local_task(notify, task_name.as_str(), f)
        });
    }

    #[async_backtrace::framed]
    async fn run_main_loop(
        name: String,
        node: Arc<NodeContext<M>>,
        channel: EventReceiver<M>,
        handle: Arc<H>,
        enable_testing:bool
    ) {
        trace!("node {}, run main loop, {}", name, node.name());
        let mut receiver = channel;

        loop {
            trace!("node {}, handle event ...", node.name());
            let opt = receiver.recv().await;
            match opt {
                Ok(event) => {
                    let h = handle.clone();
                    let _r = Self::handle_event(
                        node.clone(), event, h, enable_testing)
                        .instrument(trace_span!("handle_event")).await;
                    match _r {
                        Ok(_) => {}
                        Err(e) => {
                            match e {
                                ET::EOF => {
                                    break;
                                }
                                _ => { error!("{}", e.to_string()); }
                            }
                        }
                    }
                    trace!("node {}, handle event done...", node.name());
                }
                Err(e) => {
                    error!("error when handle event : {}", e.to_string());
                }
            }
        }
        receiver.close();
        trace!("{} {} end main loop", node.name(), name);
    }

    #[async_backtrace::framed]
    async fn handle_event(
        node: Arc<NodeContext<M>>,
        event: NetEvent<M>,
        handle: Arc<H>,
        enable_testing:bool
    ) -> Res<()> {
        match event {
            NetEvent::NetConnect {
                node_id,
                return_endpoint,
                address,
                opt_sender,
            } => {
                let id = node.name().clone();
                trace!("node {}: handle event: connect {}", id, node_id);
                Self::handle_event_connect(
                    node,
                    return_endpoint,
                    node_id,
                    address,
                    handle,
                    opt_sender,
                    enable_testing
                );
                trace!("node {}: handle event:connect {} done", id, node_id);
            }
            NetEvent::NetListen(address, opt_s) => {
                let id = node.name().clone();
                trace!("node {}: handle event: listen {}", id, address.to_string());
                let _ = Self::handle_event_listen_and_accept(
                    node,
                    address,
                    handle,
                    opt_s,
                    enable_testing
                );
                trace!("node {}: handle event: listen {} done", id, address.to_string());
            }
            NetEvent::NetSend(message, result) => {
                let node_id = message.dest();
                Self::handle_send_message(
                    node,
                    node_id,
                    false,
                    message,
                    result
                ).await?;
            }
            NetEvent::Stop(opt_s) => {
                let stop_notify = node.stop_notify();
                let _ = spawn_local_task(stop_notify, "stop and notify", async move {
                    node.stop_and_notify().await;
                    handle.on_stop().await;
                    Self::handle_opt_send_result(None, Some(Ok(())), opt_s);
                })?;
                return Err(ET::EOF);
            }
            NetEvent::NewEventChannel(ch) => {
                Self::handle_new_event_channel(
                    ch.name().clone(),
                    node.clone(),
                    ch.receiver().unwrap(),
                    handle.clone(),
                    enable_testing
                ).await?;
            }
        }
        Ok(())
    }

    #[async_backtrace::framed]
    async fn handle_send_message(
        node: Arc<NodeContext<M>>,
        node_id: NID,
        return_endpoint: bool,
        message: Message<M>,
        result_sender: ResultSenderType<
            Res<Option<Arc<dyn EndpointSync<M>>>>,
            Res<Option<Arc<dyn EndpointAsync<M>>>>
        >
    ) -> Res<()> {
        let _m = message.clone();
        let ep_result = node.get_endpoint(node_id).await;
        let ep_result = match ep_result {
            Ok(e) => {
                e.send(message).await?;
                Ok(e)
            }
            Err(e) => {
                Err(e)
            }
        };
        let (s_r, a_r) = Self::handle_result_endpoint(&node, return_endpoint, ep_result, &result_sender);
        Self::handle_opt_send_result(s_r, a_r, result_sender);
        Ok(())
    }

    #[async_backtrace::framed]
    async fn handle_new_event_channel(
        name: String,
        node: Arc<NodeContext<M>>,
        channel: EventReceiver<M>,
        handle: Arc<H>,
        enable_testing:bool
    ) -> Res<()> {
        let notify = node.stop_notify();
        let task_name = format!("main loop  {}", name);
        let main_loop = async move {
            Self::run_main_loop(
                name,
                node,
                channel,
                handle,
                enable_testing
            ).await;
        };
        spawn_local_task(notify, task_name.as_str(), main_loop)?;
        Ok(())
    }

    #[async_backtrace::framed]
    fn handle_event_connect(
        node: Arc<NodeContext<M>>,
        return_endpoint: bool,
        node_id: NID,
        address: SocketAddr,
        handle: Arc<H>,
        opt_sender: ResultSenderType<
            Res<Option<Arc<dyn EndpointSync<M>>>>,
            Res<Option<Arc<dyn EndpointAsync<M>>>>
        >,
        enable_testing:bool
    ) {
        let node_name = node.name().clone();
        let notify = node.stop_notify();
        // future process message
        let task_name = format!("{} handle connect to {} {}", node_name, node_id, address.to_string());
        let task_name2 = task_name.clone();
        let on_connected = async move {
            Self::task_handle_connected(
                node, return_endpoint, node_id,
                address, handle, opt_sender,
                enable_testing
            ).await;
            trace!("on connected done {}", task_name2);
        };
        spawn_local_task(
            notify,
            task_name.as_str(),
            on_connected,
        ).unwrap();
    }

    async fn task_handle_connected(
        node: Arc<NodeContext<M>>,
        return_endpoint: bool,
        node_id: NID,
        address: SocketAddr,
        handle: Arc<H>,
        opt_sender: ResultSenderType<
            Res<Option<Arc<dyn EndpointSync<M>>>>,
            Res<Option<Arc<dyn EndpointAsync<M>>>>
        >,
        enable_testing:bool
    ) {
        trace!("{} task handle connect to {} {}", node.name(), node_id, address.to_string());
        let r_connect = TcpStream::connect(address).await;
        trace!("{} task handle connect done, to {} {} ", node.name(), node_id, address.to_string());

        let result_endpoint = {
            match res_io(r_connect) {
                Ok(s) => {
                    let r_addr = s.peer_addr();
                    match res_io(r_addr) {
                        Ok(addr) => {
                            let opt = OptEP::new().enable_dtm_test(enable_testing);
                            let ep: Arc<dyn EndpointAsync<M>> = Arc::new(EndpointAsyncImpl::new(s, addr, opt));
                            if !return_endpoint {
                                let r = node.add_endpoint(node_id, ep.clone()).await;
                                match r {
                                    Ok(()) => { Ok(ep) }
                                    Err(e) => { Err(e) }
                                }
                            } else {
                                Ok(ep)
                            }
                        }
                        Err(_e) => { Err(_e) }
                    }
                }
                Err(e) => {
                    Err(e)
                }
            }
        };
        match handle.on_connected(
            address,
            result_endpoint.clone()).await {
            Ok(_) => {}
            Err(e) => {
                handle.on_error(e).await;
            }
        };
        let (s_r, a_r) = Self::handle_result_endpoint(&node, return_endpoint, result_endpoint, &opt_sender);
        Self::handle_opt_send_result(s_r, a_r, opt_sender);
        trace!("{} task handle connect done, on connected, to {} {} ", node.name(), node_id, address.to_string());
    }

    #[async_backtrace::framed]
    fn handle_event_listen_and_accept(
        node: Arc<NodeContext<M>>,
        address: SocketAddr,
        handle: Arc<H>,
        opt_sender: ResultSenderType<
            Res<Option<Arc<dyn EndpointSync<M>>>>,
            Res<Option<Arc<dyn EndpointAsync<M>>>>
        >,
        enable_testing:bool
    ) -> Res<()> {
        let node_id = node.node_id();
        let h = handle.clone();
        let notify = node.stop_notify();
        let future_accept_first = async move {
            trace!("bind address {}", address.to_string());
            let r_bind = TcpListener::bind(address.to_string()).await;
            let listener = match res_io(r_bind) {
                Ok(l) => {
                    Self::handle_opt_send_result(Some(Ok(None)), Some(Ok(None)), opt_sender);
                    l
                }
                Err(e) => {
                    h.on_error(e.clone()).await;
                    Self::handle_opt_send_result(Some(Ok(None)), Some(Ok(None)), opt_sender);
                    return;
                }
            };

            match Self::accept_new_connection(
                node,
                listener,
                h.clone(),
                enable_testing
            ).await {
                Ok(()) => {}
                Err(e) => {
                    h.on_error(e.clone()).await;
                }
            };
        };
        spawn_local_task(
            notify,
            format!("first accept {}", node_id).as_str(),
            future_accept_first,
        )?;
        Ok(())
    }

    async fn after_accept_connection(
        node: Arc<NodeContext<M>>,
        listener: TcpListener,
        handle: Arc<H>,
        socket: TcpStream,
        addr: SocketAddr,
        enable_testing:bool
    ) -> Res<()> {
        trace!("accept new {}", addr.to_string());
        let ep = Arc::new(EndpointAsyncImpl::new(
            socket,
            addr,
            OptEP::default().enable_dtm_test(enable_testing)
        ));
        let on_accepted = {
            let h = handle.clone();
            async move {
                match h.on_accepted(ep.clone()).await {
                    Ok(_) => {}
                    Err(e) => {
                        match e {
                            ET::EOF => {
                                trace!("connection eof")
                            }
                            _ => { h.on_error(e).await; }
                        }
                    }
                };
            }
        };

        let future_accept_new_connection = {
            let h = handle.clone();
            let n = node.clone();
            async move {
                match Self::accept_new_connection(
                    n,
                    listener,
                    h.clone(),
                    enable_testing
                ).await {
                    Err(e) => {
                        match e {
                            ET::EOF => {
                                return;
                            }
                            _ => {
                                h.on_error(e).await;
                            }
                        }
                    }
                    _ => {}
                }
            }
        };
        spawn_local_task(
            node.stop_notify(),
            format!("accept connect {}", node.name()).as_str(),
            on_accepted,
        )?;
        spawn_local_task(
            node.stop_notify(),
            format!("accept new connect {}", node.name()).as_str(),
            future_accept_new_connection,
        )?;
        Ok(())
    }

    async fn accept_new_connection(
        node: Arc<NodeContext<M>>,
        listener: TcpListener,
        handle: Arc<H>,
        enable_testing:bool
    ) -> Res<()> {
        let r = listener.accept().await;
        let (socket, addr) = res_io(r)?;
        Self::after_accept_connection(
            node,
            listener,
            handle,
            socket,
            addr,
            enable_testing
        ).await
    }


    fn handle_opt_send_result<S, A>(
        opt_ep_sync: Option<S>,
        opt_ep_async: Option<A>,
        opt_sender: ResultSenderType<S, A>) {
        match opt_sender {
            ResultSenderType::SendNone => {}
            ResultSenderType::Sync(s) => {
                match opt_ep_sync {
                    Some(ep_sync) => {
                        let r = s.send(ep_sync);
                        match r {
                            Ok(()) => {}
                            Err(_e) => { error!("send sync error"); }
                        }
                    }
                    _ => {}
                }
            }
            ResultSenderType::Async(s) => {
                match opt_ep_async {
                    Some(ep_async) => {
                        let r = s.send(ep_async);
                        match r {
                            Ok(()) => {}
                            Err(_e) => { error!("send async error"); }
                        }
                    },
                    _ => {}
                }
            }
        }
    }

    fn handle_result_endpoint(
        node: &NodeContext<M>,
        return_endpoint: bool,
        res_endpoint: Res<Arc<dyn EndpointAsync<M>>>,
        opt_result_sender: &ResultSenderType<
            Res<Option<Arc<dyn EndpointSync<M>>>>,
            Res<Option<Arc<dyn EndpointAsync<M>>>>
        >) -> (Option<Res<Option<Arc<dyn EndpointSync<M>>>>>,
               Option<Res<Option<Arc<dyn EndpointAsync<M>>>>>) {
        match opt_result_sender {
            ResultSenderType::SendNone => {
                (None, None)
            }
            ResultSenderType::Async(_) => {
                (None,
                 Some(
                     res_endpoint.map(
                         |e| {
                             if return_endpoint {
                                 Some(e)
                             } else {
                                 None
                             }
                         })
                 )
                )
            }
            ResultSenderType::Sync(_) => {
                (Some(
                    res_endpoint.map(
                        |e| -> Option<Arc<dyn EndpointSync<_>>>
                            {
                                if return_endpoint {
                                    let ep = Arc::new(EndpointSyncImpl::new(e));
                                    EndpointSyncImpl::handle_loop(ep.clone(), node.stop_notify());
                                    Some(ep)
                                } else {
                                    None
                                }
                            })
                ),
                 None)
            }
        }
    }

    fn node_event_sink(&self) -> NodeSender<M> {
        let ch = self.node_context.default_event_channel();
        NodeSender::new(ch.name().clone(), ch.sender().clone())
    }
}
