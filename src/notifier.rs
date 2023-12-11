use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::atomic::Ordering::SeqCst;
use std::thread;
use std::time::Duration;

use tokio::sync::Notify;
use tokio::task;
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

        let _r = task::spawn(async move {
            inner.task_repeat_notify_until_no_waiting().await;
        });
        ret
    }

    pub fn notify_all(&self) -> bool {
        trace!("notify waiter {}", self.name);
        let ret = self.inner.notify_all();
        let inner = self.inner.clone();
        let _s = thread::Builder::new().spawn(move || {
            inner.repeat_notify_until_no_waiting();
        });
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
    async fn task_repeat_notify_until_no_waiting(&self) {
        while self.num_waiting.load(SeqCst) != 0 {
            sleep(Duration::from_millis(10)).await;
            self.stop_notifier.notify_waiters();
        }
    }

    fn repeat_notify_until_no_waiting(&self) {
        while self.num_waiting.load(SeqCst) != 0 {
            thread::sleep(Duration::from_millis(10));
            self.stop_notifier.notify_waiters();
        }
    }
}