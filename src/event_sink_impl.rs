use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use scupt_util::error_type::ET;
use scupt_util::error_type::ET::NoneOption;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, trace};

use crate::endpoint_async::EndpointAsync;
use crate::endpoint_sync::EndpointSync;
use crate::es_option::{ESConnectOpt, ESServeOpt, ESStopOpt};
use crate::event::{AsyncReceiver, NetEvent, ResultSenderType, SyncReceiver};
use crate::event_sink_async::EventSinkAsync;
use crate::event_sink_sync::EventSinkSync;
use crate::message_receiver_async::ReceiverResp;
use crate::message_receiver_endpoint::MessageReceiverEndpoint;
use crate::message_sender_async::{SenderAsync, SenderRRAsync};
use crate::message_sender_sync::SenderSync;
use crate::opt_send::OptSend;
use crate::task_trace;

pub struct EventSenderImpl<M: MsgTrait> {
    name: String,
    sender: mpsc::UnboundedSender<NetEvent<M>>,
}


impl<M: MsgTrait> EventSenderImpl<M> {
    pub fn new(name: String, sender: mpsc::UnboundedSender<NetEvent<M>>) -> Self {
        Self {
            name,
            sender,
        }
    }

    pub fn channel_name(&self) -> &String {
        &self.name
    }

    #[async_backtrace::framed]
    async fn recv_result(
        &self,
        receiver: AsyncReceiver<Res<()>>,
    ) -> Res<()> {
        let _ = task_trace!();
        let ret = receiver.await.map_err(|e| {
            ET::RecvError(e.to_string())
        })?;
        ret
    }

    fn recv_result_sync(
        &self,
        receiver: SyncReceiver<Res<()>>,
    ) -> Res<()> {
        let ret = receiver.recv().map_err(|e| {
            ET::RecvError(e.to_string())
        })?;
        ret
    }

    #[async_backtrace::framed]
    async fn recv_result_async_ep(
        &self,
        receiver: AsyncReceiver<Res<Option<Arc<dyn EndpointAsync<M>>>>>,
    ) -> Res<Option<Arc<dyn EndpointAsync<M>>>> {
        let _ = task_trace!();
        let ret = receiver.await.map_err(|e| {
            ET::RecvError(e.to_string())
        })?;
        ret
    }

    fn recv_result_sync_ep(
        &self,
        receiver: SyncReceiver<Res<Option<Arc<dyn EndpointSync<M>>>>>,
    ) -> Res<Option<Arc<dyn EndpointSync<M>>>> {
        let ret = receiver.recv().map_err(|e| {
            ET::RecvError(e.to_string())
        })?;
        ret
    }

    #[async_backtrace::framed]
    async fn serve_async(&self, addr: SocketAddr, no_wait: bool) -> Res<()> {
        let _ = task_trace!();
        trace!("async serve {} {}", self.channel_name(), addr.to_string());
        if no_wait {
            let event = NetEvent::NetListen(addr, ResultSenderType::SendNone);
            self.async_event(event)?;
        } else {
            let (s, r) = oneshot::channel();
            let _event = NetEvent::NetListen(addr, ResultSenderType::Async(s));
            self.async_event(_event)?;
            let _r = self.recv_result_async_ep(r).await?;
        }
        Ok(())
    }

    fn serve_sync(&self, addr: SocketAddr, no_wait: bool) -> Res<()> {
        trace!("async serve {} {}", self.channel_name(), addr.to_string());
        if no_wait {
            let event = NetEvent::NetListen(addr, ResultSenderType::SendNone);
            self.async_event(event)?;
        } else {
            let (s, r) = std::sync::mpsc::channel();
            let _event = NetEvent::NetListen(addr, ResultSenderType::Sync(s));
            self.async_event(_event)?;
            let _r = self.recv_result_sync_ep(r)?;
        }
        Ok(())
    }

    #[async_backtrace::framed]
    async fn connect_async(
        &self,
        node_id: NID, address: SocketAddr,
        no_wait: bool,
        read_endpoint: bool,
    ) -> Res<Option<Arc<dyn EndpointAsync<M>>>> {
        let _ = task_trace!();
        trace!("channel name {}, send connect to {}", self.name, node_id);
        if no_wait && !read_endpoint {
            let event = NetEvent::NetConnect {
                node_id,
                return_endpoint: false,
                address,
                opt_sender: ResultSenderType::SendNone,
            };
            self.async_event(event)?;
            Ok(None)
        } else {
            let (s, r) = oneshot::channel();
            let event = NetEvent::NetConnect {
                node_id,
                return_endpoint: read_endpoint,
                address,
                opt_sender: ResultSenderType::Async(s),
            };
            self.async_event(event)?;
            let _r = self.recv_result_async_ep(r).await?;
            Ok(_r)
        }
    }

    fn connect_sync(
        &self,
        node_id: NID, address: SocketAddr,
        no_wait: bool,
        read_endpoint: bool,
    ) -> Res<Option<Arc<dyn EndpointSync<M>>>> {
        trace!("channel name {}, send connect to {}", self.name, node_id);
        if no_wait && !read_endpoint {
            let event = NetEvent::NetConnect {
                node_id,
                return_endpoint: false,
                address,
                opt_sender: ResultSenderType::SendNone,
            };
            self.async_event(event)?;
            Ok(None)
        } else {
            let (s, r) = std::sync::mpsc::channel();
            let event = NetEvent::NetConnect {
                node_id,
                return_endpoint: read_endpoint,
                address,
                opt_sender: ResultSenderType::Sync(s),
            };
            self.async_event(event)?;
            let _r = self.recv_result_sync_ep(r)?;
            Ok(_r)
        }
    }

    #[async_backtrace::framed]
    pub async fn send_async(
        &self,
        msg: Message<M>,
        no_wait: bool,
        read_resp: bool,
    ) -> Res<Option<Arc<dyn ReceiverResp<M>>>> {
        trace!("channel name {} send message {:?}", self.name, msg);
        if no_wait && !read_resp {
            let event = NetEvent::NetSend(msg, ResultSenderType::SendNone);
            self.async_event(event)?;
            trace!("channel name {} send message", self.name);
            Ok(None)
        } else {
            let (s, r) = oneshot::channel();
            let event = NetEvent::NetSend(msg, ResultSenderType::Async(s));
            self.async_event(event)?;
            let recv_r = self.recv_result_async_ep(r).await?;
            let ret = recv_r.map(|e| -> Arc<dyn ReceiverResp<M>> {
                Arc::new(MessageReceiverEndpoint::new(e))
            });
            Ok(ret)
        }
    }


    pub fn send_sync(
        &self,
        msg: Message<M>,
        no_wait: bool,
    ) -> Res<()> {
        trace!("channel name {} send message {:?}", self.name, msg);
        if no_wait {
            let event = NetEvent::NetSend(msg, ResultSenderType::SendNone);
            self.async_event(event)?;
            trace!("channel name {} send message", self.name);
            Ok(())
        } else {
            let (s, r) = std::sync::mpsc::channel();
            let event = NetEvent::NetSend(msg, ResultSenderType::Sync(s));
            self.async_event(event)?;
            let _ = self.recv_result_sync_ep(r)?;
            Ok(())
        }
    }

    #[async_backtrace::framed]
    pub async fn stop_async(&self, no_wait: bool) -> Res<()> {
        if no_wait {
            let event = NetEvent::Stop(ResultSenderType::SendNone);
            self.async_event(event)
        } else {
            let (s, r) = oneshot::channel();
            let event = NetEvent::Stop(ResultSenderType::Async(s));
            self.async_event(event)?;
            let _ = self.recv_result(r).await?;
            Ok(())
        }
    }

    pub fn stop_sync(&self, no_wait: bool) -> Res<()> {
        if no_wait {
            let event = NetEvent::Stop(ResultSenderType::SendNone);
            self.async_event(event)
        } else {
            let (s, r) = std::sync::mpsc::channel();
            let event = NetEvent::Stop(ResultSenderType::Sync(s));
            self.async_event(event)?;
            let _ = self.recv_result_sync(r)?;
            Ok(())
        }
    }

    fn async_event(&self, event: NetEvent<M>) -> Res<()> {
        let r_send = self.sender.send(event);
        match r_send {
            Ok(_) => { Ok(()) }
            Err(e) => {
                error!("send event {:?}, error {}", &e.0, e.to_string());
                Err(ET::TokioSenderError(e.to_string()))
            }
        }
    }
}


impl<M: MsgTrait + 'static> Clone for EventSenderImpl<M> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.to_string(),
            sender: self.sender.clone(),
        }
    }
}

#[async_trait]
impl<
    M: MsgTrait + 'static,
> EventSinkAsync<M> for EventSenderImpl<
    M
> {
    #[async_backtrace::framed]
    async fn stop(&self, opt: ESStopOpt) -> Res<()> {
        let _ = task_trace!();
        self.stop_async(opt.no_wait()).await
    }

    #[async_backtrace::framed]
    async fn serve(&self, addr: SocketAddr, opt: ESServeOpt) -> Res<()> {
        self.serve_async(addr, opt.no_wait()).await
    }

    #[async_backtrace::framed]
    async fn connect(&self, node_id: NID, address: SocketAddr, opt: ESConnectOpt) -> Res<Option<Arc<dyn EndpointAsync<M>>>> {
        let _t = task_trace!();
        self.connect_async(node_id, address, opt.no_wait(), opt.return_endpoint()).await
    }
}


impl<
    M: MsgTrait + 'static,
> EventSinkSync<M> for EventSenderImpl<
    M
> {
    fn stop(&self, opt: ESStopOpt) -> Res<()> {
        self.stop_sync(opt.no_wait())
    }

    fn serve(&self, addr: SocketAddr, opt: ESServeOpt) -> Res<()> {
        self.serve_sync(addr, opt.no_wait())
    }

    fn connect(&self, node_id: NID, address: SocketAddr, opt: ESConnectOpt) -> Res<Option<Arc<dyn EndpointSync<M>>>> {
        self.connect_sync(node_id, address, opt.no_wait(), opt.return_endpoint())
    }
}

#[async_trait]
impl<
    M: MsgTrait + 'static,
> SenderAsync<M> for EventSenderImpl<
    M> {
    async fn send(&self, message: Message<M>, opt: OptSend) -> Res<()> {
        let _ = self.send_sync(message, opt.is_enable_no_wait());
        Ok(())
    }
}


impl<
    M: MsgTrait + 'static,
> SenderSync<M> for EventSenderImpl<
    M> {
    fn send(&self, message: Message<M>, opt: OptSend) -> Res<()> {
        let _ = self.send_sync(message, opt.is_enable_no_wait())?;
        Ok(())
    }
}

#[async_trait]
impl<
    M: MsgTrait + 'static,
> SenderRRAsync<M> for EventSenderImpl<M> {
    #[async_backtrace::framed]
    async fn send(&self, message: Message<M>, _opt: OptSend) -> Res<Arc<dyn ReceiverResp<M>>> {
        let _t = task_trace!();
        let opt = self.send_async(message, _opt.is_enable_no_wait(), true).await?;
        match opt {
            Some(recv) => { Ok(recv) }
            None => { return Err(NoneOption); }
        }
    }
}
