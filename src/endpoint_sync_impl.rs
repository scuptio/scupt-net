use std::net::SocketAddr;
use std::sync::Arc;

use std::sync::mpsc::Sender as SyncSender;

use futures::executor::block_on;
use scupt_util::error_type::ET;
use scupt_util::message::{Message, MsgTrait};
use scupt_util::res::Res;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender as AsyncSender};
use tokio::sync::mpsc::UnboundedReceiver as AsyncReceiver;
use tokio::sync::Mutex;
use tracing::error;

use crate::endpoint_async::EndpointAsync;
use crate::endpoint_sync::EndpointSync;
use crate::notifier::Notifier;
use crate::task::spawn_local_task;

pub struct EndpointSyncImpl<M: MsgTrait + 'static> {
    endpoint: Arc<dyn EndpointAsync<M>>,
    s_sender: AsyncSender<Message<M>>,
    s_receiver: Mutex<Option<AsyncReceiver<Message<M>>>>,
    r_invoke_sender: AsyncSender<SyncSender<Message<M>>>,
    r_invoke_receiver: Mutex<Option<AsyncReceiver<SyncSender<Message<M>>>>>,
}

impl<M: MsgTrait + 'static> EndpointSync<M> for EndpointSyncImpl<M> {
    fn remote_address(&self) -> SocketAddr {
        self.endpoint.remote_address()
    }

    fn send(&self, m: Message<M>) -> Res<()> {
        self.s_sender.send(m).unwrap();
        Ok(())
    }

    fn recv(&self) -> Res<Message<M>> {
        let (s, r) = std::sync::mpsc::channel::<Message<M>>();
        let _ = self.r_invoke_sender.send(s).map_err(|e| {
            ET::SenderError(e.to_string())
        })?;
        let message = r.recv().unwrap();
        Ok(message)
    }

    fn close(&self) -> Res<()> {
        let ep = self.endpoint.clone();
        block_on(async move {
            let _ = ep.close().await;
        });
        Ok(())
    }
}

unsafe impl<M: MsgTrait + 'static> Send for EndpointSyncImpl<M> {}

unsafe impl<M: MsgTrait + 'static> Sync for EndpointSyncImpl<M> {}

impl<M: MsgTrait + 'static> EndpointSyncImpl<M> {
    pub fn new(ep: Arc<dyn EndpointAsync<M>>) -> Self {
        let (s_sender, s_receiver) = unbounded_channel();
        let (r_sender, r_receiver) = unbounded_channel();
        Self {
            endpoint: ep,
            s_sender,
            s_receiver: Mutex::new(Some(s_receiver)),
            r_invoke_sender: r_sender,
            r_invoke_receiver: Mutex::new(Some(r_receiver)),
        }
    }

    async fn handle_send(&self) -> Res<()> {
        let mut opt = self.s_receiver.lock().await;
        let mut opt_receiver = None;
        std::mem::swap(&mut opt_receiver, &mut opt);
        let mut receiver = opt_receiver.unwrap();
        loop {
            let opt_m = receiver.recv().await;
            match opt_m {
                None => { break; }
                Some(m) => {
                    self.endpoint.send(m).await?;
                }
            }
        }
        Ok(())
    }

    async fn handle_receive(&self) -> Res<()> {
        let mut opt = self.r_invoke_receiver.lock().await;
        let mut opt_receiver = None;
        std::mem::swap(&mut opt_receiver, &mut opt);
        let mut receiver = opt_receiver.unwrap();
        loop {
            let opt_channel = receiver.recv().await;
            let channel = match opt_channel {
                Some(c) => { c}
                None => { break; }
            };
            let r = self.endpoint.recv().await;
            match r {
                Ok(message) => {
                    channel.send(message).map_err(|e| {
                        ET::SenderError(e.to_string())
                    })?;
                }
                Err(e) => {
                    error!("error {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    pub fn handle_loop(s: Arc<Self>, notifier: Notifier) {
        let s1 = s.clone();
        let _r1 = spawn_local_task(notifier.clone(), "handle receive", async move {
            s1.handle_receive().await
        });
        let _r2 = spawn_local_task(notifier.clone(), "handle send", async move {
            s.handle_send().await
        });
    }
}