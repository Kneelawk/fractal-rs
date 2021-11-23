//! Utilities specifically related to Futures.

pub mod future_wrapper;

use futures::task::Context;
use std::{future::Future, pin::Pin, task::Poll};
use tokio::{
    runtime::Handle,
    task::{JoinError, JoinHandle},
};

/// Polls a `Future` that is `Unpin` in a tokio runtime referenced by `handle`.
pub fn poll_unpin<R, F: Future<Output = R> + Unpin>(handle: &Handle, future: &mut F) -> Poll<R> {
    let _enter_guard = handle.enter();

    let noop_waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&noop_waker);

    Pin::new(future).poll(&mut cx)
}

/// Specifically polls a `JoinHandle` that returns a `Result` who's error can be
/// converted from a `JoinError`.
pub fn poll_join_result<R, E>(
    handle: &Handle,
    future: &mut JoinHandle<Result<R, E>>,
) -> Option<Result<R, E>>
where
    E: From<JoinError>,
{
    if let Poll::Ready(res) = poll_unpin(handle, future) {
        Some(match res {
            Ok(res) => res,
            Err(err) => Err(E::from(err)),
        })
    } else {
        None
    }
}

/// Polls a `JoinHandle` in a `Option` attempting to insert a new `JoinHandle`
/// when the previous one completes.
pub fn poll_optional<R, N: FnOnce() -> Option<JoinHandle<anyhow::Result<R>>>>(
    handle: &Handle,
    optional_future: &mut Option<JoinHandle<anyhow::Result<R>>>,
    on_new: N,
) -> Option<anyhow::Result<R>> {
    let mut res = None;
    if let Some(future) = optional_future {
        res = poll_join_result(handle, future);
    }

    if res.is_some() {
        *optional_future = None;
    }

    if optional_future.is_none() {
        if let Some(future) = on_new() {
            *optional_future = Some(future);
        }
    }

    res
}

/// Describes something that can be in a `NotStarted` state, a `Starting` state
/// which is a `Future`, or a `Running` state.
pub enum RunningState<I, F: Future> {
    NotStarted,
    Starting(F),
    Running(I),
}

impl<I, F: Future> RunningState<I, F> {
    pub fn is_started(&self) -> bool {
        match self {
            RunningState::NotStarted => false,
            RunningState::Starting(_) => true,
            RunningState::Running(_) => true,
        }
    }
}

impl<I> RunningState<I, JoinHandle<anyhow::Result<I>>> {
    /// Attempts to convert a `Starting` state into a `Running` state by polling
    /// the starting future.
    pub fn poll_starting(&mut self, handle: &Handle) -> anyhow::Result<()> {
        let mut res = None;
        if let RunningState::Starting(f) = self {
            res = poll_join_result(handle, f);
        }
        match res {
            Some(Ok(inst)) => {
                *self = RunningState::Running(inst);
            },
            Some(Err(e)) => {
                *self = RunningState::NotStarted;
                return Err(e);
            },
            _ => {},
        }
        Ok(())
    }
}
