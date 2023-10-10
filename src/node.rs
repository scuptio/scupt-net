use std::net::SocketAddr;
use std::sync::{Arc, Once};

use scupt_util::error_type::ET;
use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use scupt_util::res_of::res_io;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::task::LocalSet;
use tracing::{error, trace, trace_span, Instrument};

use crate::endpoint::Endpoint;
use crate::event::{EventResult, NetEvent, ResultSender};
use crate::event_channel::EventReceiver;
use crate::event_sink::EventSink;
use crate::handle_event::HandleEvent;
use crate::message_sender::MessageSender;
use crate::net_handler::NodeSender;
use crate::node_context::NodeContext;
use crate::notifier::{Notifier, spawn_cancelable_task, spawn_cancelable_task_local_set};
use crate::opt_ep::OptEP;

#[derive(Clone)]
pub struct Node<
    M: MsgTrait + 'static,
    H: HandleEvent + 'static
> {
    _node_id: NID,
    handle: Arc<H>,
    node_context: Arc<NodeContext<M>>,
    run_once: Arc<Once>,
}


impl<
    M: MsgTrait + 'static,
    H: HandleEvent + 'static
>
Node<
    M,
    H
> {
    pub fn new(
        node_id: NID,
        name: String,
        handle: H,
        stop_notify: Notifier,
    ) -> Res<Self> {
        let node_context = NodeContext::new(node_id.clone(), name, stop_notify);


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

    pub fn new_event_channel(&self, name: String) -> Res<Arc<dyn EventSink>> {
        self.node_context.new_event_channel(name)
    }
    pub fn new_message_sender(&self, name: String) -> Res<Arc<dyn MessageSender<M>>> {
        self.node_context.new_message_sender(name)
    }

    pub fn stop_notify(&self) -> Notifier {
        self.node_context.stop_notify()
    }

    pub fn default_event_sink(&self) -> Arc<dyn EventSink> {
        Arc::new(self.node_event_sender())
    }

    pub fn default_message_sender(&self) -> Arc<dyn MessageSender<M>> {
        Arc::new(self.node_event_sender())
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
        let c = self.node_context.default_event_channel().receiver().unwrap();
        trace!("main loop {}", n.name());
        let task_name = format!("{}_main_loop", n.name());
        let notify = n.stop_notify();
        spawn_cancelable_task_local_set(
            &local_set,
            notify,
            task_name.as_str(),
            async move {
                Self::run_main_loop(
                    name,
                    n,
                    c,
                    h,
                ).instrument(trace_span!("main loop")).await;
            },
        );
    }

    #[async_backtrace::framed]
    async fn run_main_loop(
        name: String,
        node: Arc<NodeContext<M>>,
        channel: EventReceiver<M>,
        handle: Arc<H>,
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
                        node.clone(), event, h)
                        .instrument(trace_span!("handle_event")).await;
                    match _r {
                        Ok(_) => {}
                        Err(e) => {
                            match e {
                                ET::StopService => {
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
    ) -> Res<()> {
        match event {
            NetEvent::NetConnect {
                node_id,
                address,
                opt_sender,
                return_endpoint
            } => {
                let id = node.name().clone();
                trace!("node {}: handle event: connect {}", id, node_id);
                Self::handle_event_connect(
                    node,
                    node_id,
                    address,
                    handle,
                    opt_sender,
                    return_endpoint,
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
                );
                trace!("node {}: handle event: listen {} done", id, address.to_string());
            }
            NetEvent::NetSend(node_id, message, opt_s) => {
                trace!("handle event: send {:?}", message);
                let r = Self::handle_send_message(
                    node,
                    node_id,
                    message).await;
                Self::handle_opt_send_result(EventResult::ErrorType(r.err().unwrap_or(ET::OK)), opt_s);
                trace!("handle event: send done");
            }
            NetEvent::Stop(opt_s) => {
                let stop_notify = node.stop_notify();
                spawn_cancelable_task(stop_notify, "stop and notify", async move {
                    node.stop_and_notify().await;
                    handle.on_stop().await;
                    Self::handle_opt_send_result(EventResult::ErrorType(ET::OK), opt_s);
                });
                return Err(ET::StopService);
            }
            NetEvent::NewEventChannel(ch) => {
                Self::handle_new_event_channel(
                    ch.name().clone(),
                    node.clone(),
                    ch.receiver().unwrap(),
                    handle.clone(),
                ).await?;
            }
        }
        Ok(())
    }

    #[async_backtrace::framed]
    async fn handle_send_message(
        node: Arc<NodeContext<M>>,
        node_id: NID,
        message: M,
    ) -> Res<()> {
        let ep = node.get_endpoint(node_id).await?;
        ep.send(message).await?;
        Ok(())
    }

    #[async_backtrace::framed]
    async fn handle_new_event_channel(
        name: String,
        node: Arc<NodeContext<M>>,
        channel: EventReceiver<M>,
        handle: Arc<H>,
    ) -> Res<()> {
        let notify = node.stop_notify();
        let task_name = format!("main loop  {}", name);
        let main_loop = async move {
            Self::run_main_loop(
                name,
                node,
                channel,
                handle,
            ).await;
        };
        spawn_cancelable_task(notify, task_name.as_str(), main_loop);
        Ok(())
    }

    #[async_backtrace::framed]
    fn handle_event_connect(
        node: Arc<NodeContext<M>>,
        node_id: NID,
        address: SocketAddr,
        handle: Arc<H>,
        opt_sender: Option<ResultSender>,
        return_endpoint: bool,
    ) {
        let node_name = node.name().clone();
        let notify = node.stop_notify();
        // future process message
        let task_name = format!("{} handle connect to {} {}", node_name, node_id, address.to_string());
        let task_name2 = task_name.clone();
        let on_connected = async move {
            Self::task_handle_connected(
                node, node_id,
                address, handle, opt_sender,
                return_endpoint,
            ).await;
            trace!("on connected done {}", task_name2);
        };
        spawn_cancelable_task(
            notify,
            task_name.as_str(),
            on_connected,
        );
    }

    async fn task_handle_connected(
        node: Arc<NodeContext<M>>,
        node_id: NID,
        address: SocketAddr,
        handle: Arc<H>,
        opt_sender: Option<ResultSender>,
        return_endpoint: bool,
    ) {
        trace!("{} task handle connect to {} {}", node.name(), node_id, address.to_string());
        let r_connect = TcpStream::connect(address).await;
        trace!("{} task handle connect done, to {} {} ", node.name(), node_id, address.to_string());

        let result = {
            match res_io(r_connect) {
                Ok(s) => {
                    let r_addr = s.peer_addr();
                    match res_io(r_addr) {
                        Ok(addr) => {
                            let ep = Endpoint::new(s, addr, OptEP::default());
                            {
                                let r = node.add_endpoint(node_id, ep.clone()).await;
                                match r {
                                    Ok(()) => { Ok(ep) }
                                    Err(e) => { Err(e) }
                                }
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
            result.clone()).await {
            Ok(_) => {}
            Err(e) => {
                handle.on_error(e).await;
            }
        };
        let er = if return_endpoint {
            match result {
                Ok(ep) => { EventResult::NetEndpoint(ep) }
                Err(e) => {
                    EventResult::ErrorType(e)
                }
            }
        } else {
            EventResult::ErrorType(result.err().unwrap_or(ET::OK))
        };
        Self::handle_opt_send_result(er, opt_sender);
        trace!("{} task handle connect done, on connected, to {} {} ", node.name(), node_id, address.to_string());
    }

    #[async_backtrace::framed]
    fn handle_event_listen_and_accept(
        node: Arc<NodeContext<M>>,
        address: SocketAddr,
        handle: Arc<H>,
        opt_sender: Option<ResultSender>,
    ) -> Res<()> {
        let node_id = node.node_id();
        let h = handle.clone();
        let notify = node.stop_notify();
        let future_accept_first = async move {
            trace!("bind address {}", address.to_string());
            let r_bind = TcpListener::bind(address.to_string()).await;
            let listener = match res_io(r_bind) {
                Ok(l) => {
                    Self::handle_opt_send_result(EventResult::ErrorType(ET::OK), opt_sender);
                    l
                }
                Err(e) => {
                    h.on_error(e.clone()).await;
                    Self::handle_opt_send_result(EventResult::ErrorType(e.clone()), opt_sender);
                    return;
                }
            };

            match Self::accept_new_connection(
                node,
                listener,
                h.clone()).await {
                Ok(()) => {}
                Err(e) => {
                    h.on_error(e.clone()).await;
                }
            };
        };
        spawn_cancelable_task(
            notify,
            format!("first accept {}", node_id).as_str(),
            future_accept_first,
        );
        Ok(())
    }

    async fn after_accept_connection(
        node: Arc<NodeContext<M>>,
        listener: TcpListener,
        handle: Arc<H>,
        socket: TcpStream,
        addr: SocketAddr,
    ) -> Res<()> {
        trace!("accept new {}", addr.to_string());
        let ep = Endpoint::new(
            socket,
            addr,
            OptEP::default().enable_dtm_test(true)
        );
        let on_accepted = {
            let h = handle.clone();
            async move {
                match h.on_accepted(ep.clone()).await {
                    Ok(_) => {}
                    Err(e) => {
                        match e {
                            ET::StopService => {
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
                ).await {
                    Err(e) => {
                        match e {
                            ET::StopService => {
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
        spawn_cancelable_task(
            node.stop_notify(),
            format!("accept connect {}", node.name()).as_str(),
            on_accepted,
        );
        spawn_cancelable_task(
            node.stop_notify(),
            format!("accept new connect {}", node.name()).as_str(),
            future_accept_new_connection,
        );
        Ok(())
    }

    async fn accept_new_connection(
        node: Arc<NodeContext<M>>,
        listener: TcpListener,
        handle: Arc<H>,
    ) -> Res<()> {
        let r = listener.accept().await;
        let (socket, addr) = res_io(r)?;
        Self::after_accept_connection(
            node,
            listener,
            handle,
            socket,
            addr,
        ).await
    }

    fn handle_opt_send_result(er: EventResult, opt_sender: Option<ResultSender>) {
        match opt_sender {
            None => {}
            Some(sender) => {
                let r = sender.send(er);
                match r {
                    Ok(()) => {}
                    Err(_e) => { error!("send result error"); }
                }
            }
        }
    }

    fn node_event_sender(&self) -> NodeSender<M> {
        let ch = self.node_context.default_event_channel();
        NodeSender::new(ch.name().clone(), ch.sender().clone())
    }
}
