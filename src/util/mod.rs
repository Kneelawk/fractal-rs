//! util.rs - Random utility functions for the program.

pub mod result;

use futures::task::Context;
use std::{future::Future, pin::Pin, task::Poll};
use tokio::task::JoinHandle;

pub fn push_or_else<T, E, F: FnOnce(E)>(res: Result<T, E>, vec: &mut Vec<T>, or_else: F) {
    match res {
        Ok(val) => vec.push(val),
        Err(err) => or_else(err),
    }
}

pub fn poll_unpin<R, F: Future<Output = R> + Unpin>(future: &mut F) -> Poll<R> {
    let noop_waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&noop_waker);

    Pin::new(future).poll(&mut cx)
}

pub fn poll_join_result<R>(
    future: &mut JoinHandle<anyhow::Result<R>>,
) -> Option<anyhow::Result<R>> {
    if let Poll::Ready(res) = poll_unpin(future) {
        Some(match res {
            Ok(res) => res,
            Err(err) => Err(anyhow::Error::from(err)),
        })
    } else {
        None
    }
}

pub fn poll_optional<R, N: FnOnce() -> Option<JoinHandle<anyhow::Result<R>>>>(
    optional_future: &mut Option<JoinHandle<anyhow::Result<R>>>,
    on_new: N,
) -> Option<anyhow::Result<R>> {
    let mut res = None;
    if let Some(future) = optional_future {
        res = poll_join_result(future);
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
    pub fn poll_starting(&mut self) -> anyhow::Result<()> {
        let mut res = None;
        if let RunningState::Starting(f) = self {
            res = poll_join_result(f);
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