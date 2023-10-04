use std::net::SocketAddr;

use async_trait::async_trait;
use scupt_util::error_type::ET;
use scupt_util::message::MsgTrait;
use scupt_util::node_id::NID;
use scupt_util::res::Res;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, trace};

use crate::endpoint::Endpoint;
use crate::event::{EventResult, NetEvent, ResultReceiver};
use crate::event_sink::{ESConnectOpt, ESServeOpt, ESStopOpt, EventSink};
use crate::message_sender::MessageSender;
use crate::opt_send::OptSend;

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

    async fn async_serve(&self, addr: SocketAddr, no_wait: bool) -> Res<()> {
        trace!("async serve {} {}", self.channel_name(), addr.to_string());
        if no_wait {
            let event = NetEvent::NetListen(addr, None);
            self.async_event(event)
        } else {
            let (s, r) = oneshot::channel::<EventResult>();
            let event = NetEvent::NetListen(addr, Some(s));
            let event_result = self.wait_send_event_result(event, r).await?;
            self.event_result(event_result)
        }
    }

    async fn async_connect(
        &self,
        node_id: NID, address: SocketAddr,
        no_wait: bool,
        return_ep: bool,
    ) -> Res<Option<Endpoint>> {
        trace!("channel name {}, send connect to {}", self.name, node_id);
        if no_wait && !return_ep {
            let event = NetEvent::NetConnect {
                node_id,
                address,
                opt_sender: None,
                return_endpoint: return_ep,
            };
            self.async_event(event)?;
            Ok(None)
        } else {
            let (s, r) = oneshot::channel::<EventResult>();
            let event = NetEvent::NetConnect {
                node_id,
                address,
                opt_sender: Some(s),
                return_endpoint: return_ep,
            };
            let event_result = self.wait_send_event_result(event, r).await?;
            if !return_ep {
                self.event_result(event_result)?;
                Ok(None)
            } else {
                let endpoint = self.event_result_endpoint(event_result)?;
                Ok(Some(endpoint))
            }
        }
    }


    #[async_backtrace::framed]
    pub async fn async_send(&self, node_id: NID, msg: M, no_wait: bool) -> Res<()> {
        trace!("channel name {} send message {:?}", self.name, msg);
        if no_wait {
            let event = NetEvent::NetSend(node_id, msg, None);
            self.async_event(event)?;
            trace!("channel name {} send message", self.name);
        } else {
            let (s, r) = oneshot::channel::<EventResult>();
            let event = NetEvent::NetSend(node_id, msg, Some(s));
            let event_result = self.wait_send_event_result(event, r).await?;
            self.event_result(event_result)?
        }

        Ok(())
    }

    pub async fn async_stop(&self, no_wait: bool) -> Res<()> {
        if no_wait {
            let event = NetEvent::Stop(None);
            self.async_event(event)
        } else {
            let (s, r) = oneshot::channel::<EventResult>();
            let event = NetEvent::Stop(Some(s));
            let event_result = self.wait_send_event_result(event, r).await?;
            self.event_result(event_result)
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

    fn event_result(&self, er: EventResult) -> Res<()> {
        match er {
            EventResult::ErrorType(e) => {
                match e {
                    ET::OK => { Ok(()) }
                    _ => { Err(e) }
                }
            }
            EventResult::NetEndpoint(_ep) => {
                panic!("");
            }
        }
    }

    fn event_result_endpoint(&self, er: EventResult) -> Res<Endpoint> {
        match er {
            EventResult::ErrorType(e) => {
                match e {
                    ET::OK => { panic!("error"); }
                    _ => { Err(e) }
                }
            }
            EventResult::NetEndpoint(ep) => {
                Ok(ep)
            }
        }
    }

    async fn wait_send_event_result(&self, event: NetEvent<M>, receiver: ResultReceiver) -> Res<EventResult> {
        self.async_event(event)?;
        trace!("send event {}", self.name);
        let r = receiver.await;
        match r {
            Ok(e) => {
                Ok(e)
            }
            Err(e) => {
                error!("{}", e.to_string());
                Err(ET::RecvError(e.to_string()))
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
> EventSink for EventSenderImpl<
    M
> {
    async fn stop(&self, opt: ESStopOpt) -> Res<()> {
        self.async_stop(opt.no_wait()).await
    }

    async fn serve(&self, addr: SocketAddr, opt: ESServeOpt) -> Res<()> {
        self.async_serve(addr, opt.no_wait()).await
    }

    async fn connect(&self, node_id: NID, address: SocketAddr, opt: ESConnectOpt) -> Res<Option<Endpoint>> {
        self.async_connect(node_id, address, opt.no_wait(), opt.return_endpoint()).await
    }
}

#[async_trait]
impl<
    M: MsgTrait + 'static,
> MessageSender<M> for EventSenderImpl<
    M> {
    #[async_backtrace::framed]
    async fn send(&self, node_id: NID, message: M, _opt: OptSend) -> Res<()> {
        self.async_send(node_id, message, true).await
    }
}

