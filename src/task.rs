use std::future::Future;
use std::time::Duration;

use scupt_util::res::Res;
use tokio::time::sleep;
use tokio::{select, task};
use tokio::task::JoinHandle;
use tracing::trace;

use crate::notifier::Notifier;

#[cfg(not(task_name))]
pub fn spawn_local_task<F>(cancel_notifier: Notifier, _name: &str, future: F) -> Res<JoinHandle<Option<F::Output>>>
    where
        F: Future + 'static,
        F::Output: 'static,
{
    Ok(task::spawn_local(async move {
        __select_local_till_done(cancel_notifier, future).await
    }))
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
pub fn spawn_task<F>(cancel_notifier: Notifier, _name: &str, future: F) -> Res<JoinHandle<Option<F::Output>>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
{
    Ok(task::spawn(async move {
        __select_till_done(cancel_notifier, future).await
    }))
}

#[cfg(not(task_name))]
pub fn spawn_local_task_timeout<F>(
    cancel_notifier: Notifier,
    duration: Duration,
    _name: &str,
    future: F
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

async fn __select_local_till_done_or_timeout<F>(notify: Notifier, duration:Duration, future: F) -> Result<F::Output, TaskFailed>
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