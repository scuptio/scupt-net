use std::future::Future;
use std::io::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::atomic::Ordering::SeqCst;
use std::thread;
use std::time::Duration;

use tokio::select;
use tokio::sync::Notify;
use tokio::task;
use tokio::task::{JoinHandle, LocalSet};
use tokio::time::sleep;
use tracing::trace;

// Notifies tasks to wake up.
// If Notifier::notify_waiters is called, all the task call Notifier::notified would complete, and
// the following invocation of Notifier::notified, which after Notifier::notify_waiters called,
// would return immediately
#[derive(Clone)]
pub struct Notifier {
    name: String,
    inner: Arc<NotifyInner>,
}

pub struct NotifyInner {
    stop_notifier: Notify,
    stopped: AtomicBool,
    num_waiting: AtomicU64,
}

impl Default for Notifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier {
    pub fn new() -> Self {
        Self {
            name: "".to_string(),
            inner: Arc::new(NotifyInner::new()),
        }
    }

    pub fn new_with_name(name: String) -> Self {
        Self {
            name,
            inner: Arc::new(NotifyInner::new()),
        }
    }

    pub fn is_notified(&self) -> bool {
        self.inner.is_notified()
    }

    pub async fn notified(&self) {
        trace!("notified {}", self.name);
        self.inner.notified().await;
        trace!("notified {} done", self.name);
    }

    pub fn task_notify_all(&self) -> bool {
        trace!("notify waiter {}", self.name);
        let ret = self.inner.notify_all();
        let inner = self.inner.clone();
        let _ = task::Builder::default().spawn(async move {
            inner.repeat_notify_until_no_waiting().await;
        }).unwrap();
        ret
    }

    pub fn notify_all(&self) -> bool {
        trace!("notify waiter {}", self.name);
        let ret = self.inner.notify_all();
        let inner = self.inner.clone();
        inner.repeat_notify();
        ret
    }
}

impl NotifyInner {
    fn new() -> Self {
        Self {
            stopped: AtomicBool::new(false),
            stop_notifier: Notify::new(),
            num_waiting: AtomicU64::new(0),
        }
    }

    async fn notified(&self) {
        self.num_waiting.fetch_add(1, Ordering::SeqCst);
        if !self.stopped.load(Ordering::SeqCst) {
            self.stop_notifier.notified().await;
        }
        self.num_waiting.fetch_sub(1, Ordering::SeqCst);
    }

    fn is_notified(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    fn notify_all(&self) -> bool {
        let r = self.stopped.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst);
        let ret = match r {
            Ok(_) => {
                self.stop_notifier.notify_waiters();
                true
            }
            Err(_) => {
                self.stop_notifier.notify_waiters();
                false
            }
        };


        ret
    }

    // Keep invoking notify_waiters until the num_waiting is 0.
    async fn repeat_notify_until_no_waiting(&self) {
        while self.num_waiting.load(SeqCst) == 0 {
            sleep(Duration::from_millis(10)).await;
            self.stop_notifier.notify_waiters();
        }
    }

    fn repeat_notify(&self) {
        while self.num_waiting.load(SeqCst) == 0 {
            thread::sleep(Duration::from_millis(10));
            self.stop_notifier.notify_waiters();
        }
    }
}

pub async fn select_till_done<F>(notify: Notifier, future: F)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
{
    let f = async move {
        let _ = select! {
            _ = notify.notified() => {
            trace ! ("task stop");
            }
            _r = future => {
            trace ! ("task  end");
            }
        };
    };
    f.await;
}


pub async fn select_local_till_done<F>(notify: Notifier, future: F)
    where
        F: Future + 'static,
        F::Output: 'static,
{
    let future = async move {
        let _ = select! {
            _ = notify.notified() => {
            trace ! ("local task stop");
            }
            _r = future => {
            trace ! ("local task  end");
            }
        };
    };
    future.await;
}

pub fn spawn_cancelable_task_mt<F>(stop_notify: Notifier, name: &str, future: F) -> Result<JoinHandle<()>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
{
    task::Builder::default().name(name).spawn(async move {
        select_till_done(stop_notify, future).await
    })
}

pub fn spawn_cancelable_task_local_set<F>(
    task_set: &LocalSet,
    stop_notify: Notifier,
    name: &str,
    future: F)
    where
        F: Future + 'static,
        F::Output: 'static,
{
    let n = String::from(name);
    task_set.spawn_local(async move {
        spawn_cancelable_task(stop_notify, n.as_str(), future)
    });
}


pub fn spawn_cancelable_task<F>(stop_notify: Notifier, name: &str, future: F)
    where
        F: Future + 'static,
        F::Output: 'static,
{
    let _ = task::Builder::default().name(name).spawn_local(async move {
        select_local_till_done(stop_notify, future).await
    }).unwrap();
}
