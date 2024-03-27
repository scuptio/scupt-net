/*
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


use crate::event::{EventResult, NetEvent, NetSendControl, ResultReceiver};
use crate::es_option::{ESConnectOpt, ESServeOpt, ESStopOpt};
use crate::event_sink::EventSink;
use crate::event_sink_async::EventSinkAsync;
use crate::message_receiver::ReceiverResp;
use crate::message_sender::{Sender, SenderRR};
use crate::message_receiver_endpoint::MessageReceiverEndpoint;

use crate::opt_send::OptSend;

pub struct EventSinkImpl<M: MsgTrait> {
    name: String,
    sender: mpsc::UnboundedSender<NetEvent<M>>,
}

impl<M: MsgTrait> EventSinkImpl<M> {
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
            let (s, r) = oneshot::channel::<EventResult<M>>();
            let event = NetEvent::NetListen(addr, Some(s));
            let event_result = self.wait_send_event_result(event, r).await?;
            let _ = self.event_result(event_result)?;
            Ok(())
        }
    }

    async fn async_connect(
        &self,
        node_id: NID, address: SocketAddr,
        no_wait: bool,
        return_ep: bool,
    ) -> Res<Option<Arc<dyn EndpointAsync<M>>>> {
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
            let (s, r) = oneshot::channel::<EventResult<M>>();
            let event = NetEvent::NetConnect {
                node_id,
                address,
                opt_sender: Some(s),
                return_endpoint: return_ep,
            };
            let event_result = self.wait_send_event_result(event, r).await?;
            if !return_ep {
                let _ = self.event_result(event_result)?;
                Ok(None)
            } else {
                let opt_ep = self.event_result_endpoint(event_result)?;
                Ok(opt_ep)
            }
        }
    }


    #[async_backtrace::framed]
    pub async fn async_send(
        &self,
        msg: Message<M>,
        no_wait: bool,
        read_resp: bool
    ) -> Res<Option<Arc<dyn ReceiverResp<M>>>> {
        trace!("channel name {} send message {:?}", self.name, msg);
        if no_wait && !read_resp {
            let ctrl = NetSendControl {
                sender:None,
                return_response: read_resp,
            };
            let event = NetEvent::NetSend(msg, ctrl);
            self.async_event(event)?;
            trace!("channel name {} send message", self.name);
            Ok(None)
        } else {
            let (s, r) = oneshot::channel::<EventResult<M>>();
            let ctrl = NetSendControl {
                sender: Some(s),
                return_response: read_resp,
            };
            let event = NetEvent::NetSend(msg, ctrl);
            let event_result = self.wait_send_event_result(event, r).await?;
            let opt_ep = self.event_result_endpoint(event_result)?;
            match opt_ep {
                Some(ep) => {
                    Ok(Some(Arc::new(MessageReceiverEndpoint::new(ep))))
                }
                None => {
                    Ok(None)
                }
            }
        }
    }

    pub async fn async_stop(&self, no_wait: bool) -> Res<()> {
        if no_wait {
            let event = NetEvent::Stop(None);
            self.async_event(event)
        } else {
            let (s, r) = oneshot::channel::<EventResult<M>>();
            let event = NetEvent::Stop(Some(s));
            let event_result = self.wait_send_event_result(event, r).await?;
            let _ = self.event_result(event_result)?;
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

    fn event_result(&self, er: EventResult<M>) -> Res<()> {
        match er {
            EventResult::ErrorType(e) => {
                match e {
                    ET::OK => { Ok(()) }
                    _ => { Err(e) }
                }
            }
            EventResult::NetEndpoint(_ep) => {
                panic!("not possible");
            }
        }
    }

    fn event_result_endpoint(&self, er: EventResult<M>) -> Res<Option<Arc<dyn EndpointAsync<M>>>> {
        match er {
            EventResult::ErrorType(e) => {
                match e {
                    ET::OK => { Ok(None) }
                    _ => { Err(e) }
                }
            }
            EventResult::NetEndpoint(ep) => {
                let e = ep?;
                Ok(Some(e))
            }
        }
    }
    async fn wait_send_event_result(&self, event: NetEvent<M>, receiver: ResultSenderType<M>) -> Res<EventResult<M>> {
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


impl<M: MsgTrait + 'static> Clone for EventSinkImpl<M> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.to_string(),
            sender: self.sender.clone(),
        }
    }
}


impl<
    M: MsgTrait + 'static,
> EventSink<M> for EventSinkImpl<
    M
> {
     fn stop(&self, opt: ESStopOpt) -> Res<()> {
        self.async_stop(opt.no_wait()).await
    }

     fn serve(&self, addr: SocketAddr, opt: ESServeOpt) -> Res<()> {
        self.async_serve(addr, opt.no_wait()).await
    }

    async fn connect(&self, node_id: NID, address: SocketAddr, opt: ESConnectOpt) -> Res<Option<Arc<dyn EndpointAsync<M>>>> {
        self.async_connect(node_id, address, opt.no_wait(), opt.return_endpoint()).await
    }
}


*/