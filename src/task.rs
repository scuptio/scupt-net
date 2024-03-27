use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use lazy_static::lazy_static;
use scc::HashIndex;

use scupt_util::res::Res;
use tokio::{select, task, task_local};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::trace;
use uuid::Uuid;

use crate::notifier::Notifier;

/// TaskID use to store async task related context
/// Any async function can have a TaskID parameter to retrieve this task context
/// If rust can support [Custom Future contexts](https://github.com/rust-lang/rfcs/issues/2900)
/// The context information can be kept in Future
pub type TaskID = u128;

task_local! {
    static TASK_ID: TaskID;
}
pub struct TaskContext {
    name: String,
    notifier:Notifier,
    local_task:bool,
    id:u128,
    location:Mutex<String>,
}

pub struct Trace {

}


impl Trace {
    pub fn new() -> Self {
        Self::enter();
        Self {

        }
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        Trace::exit()
    }
}

#[macro_export]
macro_rules! task_trace {
    (
        $id:expr
    ) => {
        {
            Trace::new($id)
        }
    };
}

pub fn this_task_id() -> TaskID {
    TASK_ID.get()
}
impl Trace {
    fn enter() {
        let _id = this_task_id();
        let opt = TaskContext::get(_id);
        match opt {
            Some(_t) => {

            }
            _ => {}
        }
    }

    fn exit() {
        let _id = this_task_id();
        let opt = TaskContext::get(_id);
        match opt {
            Some(_t) => {

            }
            _ => {}
        }
    }
}
pub fn new_task_id() -> TaskID {
    Uuid::new_v4().as_u128()
}

impl TaskContext {
    fn new_context(id:TaskID, name:String, local_task:bool, notifier: Notifier) -> Arc<Self> {
        let r = Self {
            name,
            notifier,
            local_task,
            id,
            location: Default::default(),
        };
        let ret = Arc::new(r);
        let id = ret.id();
        let _ = TASK_CONTEXT.insert(id, ret.clone());
        return ret
    }

    fn remove_context(id:TaskID) {
        let _ = TASK_CONTEXT.remove(&id);
    }

    pub fn get(id:TaskID) -> Option<Arc<TaskContext>> {
        let opt = TASK_CONTEXT.get(&id);
        opt.map(|e| {
            e.get().clone()
        })
    }

    pub fn is_local(&self) -> bool {
        self.local_task
    }

    pub fn id(&self) -> TaskID {
        self.id
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn notifier(&self) -> Notifier {
        self.notifier.clone()
    }

    pub fn enter(&self) {
        let s = async_backtrace::location!().to_string();
        let mut location = self.location.lock().unwrap();
        *location = s;
    }

    pub fn exit(&self) {
        let s = async_backtrace::location!().to_string();
        let mut location = self.location.lock().unwrap();
        *location = s;
    }
}

lazy_static! {
    static ref TASK_CONTEXT : HashIndex<TaskID, Arc<TaskContext>> = HashIndex::new();
}
#[cfg(not(task_name))]
pub fn spawn_local_task<F>(cancel_notifier: Notifier, _name: &str, future: F) -> Res<JoinHandle<Option<F::Output>>>
    where
        F: Future + 'static,
        F::Output: 'static,
{
    let id = new_task_id();
    Ok(task::spawn_local(TASK_ID.scope(id, async move {
        let r = __select_local_till_done(cancel_notifier, future).await;
        let _ = TaskContext::remove_context(id);
        r
    })))
}

#[cfg(task_name)]
pub fn spawn_local_task<F>(cancel_notifier: Notifier, name: &str, future: F) -> Res<JoinHandle<Option<F::Output>>>
    where
        F: Future + 'static,
        F::Output: 'static,
{
    let r = task::Builder::default().name(name).spawn_local(async move {
        __select_local_till_done(cancel_notifier, future).await
    });
    match r {
        Ok(f) => Ok(f),
        Err(e) => Err(scupt_util::error_type::ET::FatalError(e.to_string()))
    }
}

#[cfg(not(task_name))]
pub fn spawn_task<F>(cancel_notifier: Notifier,  _name: &str, future: F) -> Res<JoinHandle<Option<F::Output>>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
{
    let id = new_task_id();
    let _ = TaskContext::new_context(id, _name.to_string(), false, cancel_notifier.clone());
    Ok(task::spawn(TASK_ID.scope(id, async move {
        let r = __select_till_done(cancel_notifier, future).await;
        let _ = TaskContext::remove_context(id);
        r
    })))
}

#[cfg(not(task_name))]
pub fn spawn_local_task_timeout<F>(
    cancel_notifier: Notifier,
    duration: Duration,
    _name: &str,
    future: F,
) -> Res<JoinHandle<Result<F::Output, TaskFailed>>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
{
    Ok(task::spawn_local(async move {
        __select_local_till_done_or_timeout(cancel_notifier, duration, future).await
    }))
}

#[cfg(task_name)]
pub fn spawn_task<F>(cancel_notifier: Notifier, name: &str, future: F) -> Res<JoinHandle<Option<F::Output>>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
{
    let r = task::Builder::default().name(name).spawn(async move {
        __select_till_done(cancel_notifier, future).await
    });
    match r {
        Ok(f) => Ok(f),
        Err(e) => Err(scupt_util::error_type::ET::FatalError(e.to_string()))
    }
}

#[cfg(task_name)]
pub fn spawn_task<F>(cancel_notifier: Notifier, duration: Duration, name: &str, future: F) -> Res<JoinHandle<Result<F::Output, TaskFailed>>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
{
    let r = task::Builder::default().name(name).spawn(async move {
        __select_local_till_done_or_timeout(cancel_notifier, duration, future).await
    });
    match r {
        Ok(f) => Ok(f),
        Err(e) => Err(scupt_util::error_type::ET::FatalError(e.to_string()))
    }
}

async fn __select_local_till_done<F>(notify: Notifier, future: F) -> Option<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
{
    let future = async move {
        let r = select! {
            _ = notify.notified() => {
                trace ! ("local task stop");
                None
            }
            r = future => {
                trace ! ("local task  end");
                Some(r)
            }
        };
        r
    };
    let opt = future.await;
    opt
}

pub enum TaskFailed {
    Cancel,
    Timeout,
}

async fn __select_local_till_done_or_timeout<F>(notify: Notifier, duration: Duration, future: F) -> Result<F::Output, TaskFailed>
    where
        F: Future + 'static,
        F::Output: 'static,
{
    let future = async move {
        let r = select! {
            _ = notify.notified() => {
                trace ! ("local task stop");
                 Err(TaskFailed::Cancel)
            }
            r = future => {
                trace ! ("local task  end");
                Ok(r)
            }
            _ = sleep(duration) => {
                Err(TaskFailed::Timeout)
            }
        };
        r
    };
    let opt = future.await;
    opt
}

async fn __select_till_done<F>(notify: Notifier, future: F) -> Option<F::Output>
    where
        F: Future + 'static,
        F::Output: Send + 'static,
{
    let future = async move {
        let r = select! {
            _ = notify.notified() => {
                trace ! ("task stop");
                None
            }
            r = future => {
                trace ! ("task  end");
                Some(r)
            }
        };
        r
    };
    let opt = future.await;
    opt
}