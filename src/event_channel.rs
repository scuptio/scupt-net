use scupt_util::error_type::ET;
use scupt_util::message::MsgTrait;
use scupt_util::res::Res;
use tokio::sync::mpsc;
use tracing::trace;

use crate::event::NetEvent;

type SyncMutex<T> = std::sync::Mutex<T>;

pub struct EventChannel<M: MsgTrait + 'static> {
    name: String,
    ch_sender: mpsc::UnboundedSender<NetEvent<M>>,
    ch_receiver: SyncMutex<Option<EventReceiver<M>>>,
}


pub struct EventReceiver<M: MsgTrait + 'static> {
    name: String,
    inner: mpsc::UnboundedReceiver<NetEvent<M>>,
}

impl<M: MsgTrait + 'static> Drop for EventReceiver<M> {
    fn drop(&mut self) {
        trace!("drop receiver {}", self.name);
    }
}

impl<M: MsgTrait + 'static> EventReceiver<M> {
    fn new(name: String, receiver: mpsc::UnboundedReceiver<NetEvent<M>>) -> Self {
        Self {
            name,
            inner: receiver,
        }
    }

    pub async fn recv(&mut self) -> Res<NetEvent<M>> {
        let opt = self.inner.recv().await;
        trace!("{} receive event", self.name);
        match opt {
            Some(e) => Ok(e),
            None => { Err(ET::NoneOption) }
        }
    }

    pub fn close(&mut self) {
        self.inner.close()
    }
}

impl<M: MsgTrait + 'static> EventChannel<M> {
    pub fn new(name: String) -> Self {
        let (s, r) =
            mpsc::unbounded_channel::<NetEvent<M>>();
        Self {
            name: name.clone(),
            ch_sender: s,
            ch_receiver: SyncMutex::new(Some(EventReceiver::new(name.clone(), r))),
        }
    }
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn sender(&self) -> &mpsc::UnboundedSender<NetEvent<M>> {
        &self.ch_sender
    }

    pub fn receiver(&self) -> Option<EventReceiver<M>> {
        let mut r = self.ch_receiver.lock().unwrap();
        let mut ret = None;
        std::mem::swap(&mut ret, &mut r);
        ret
    }
}