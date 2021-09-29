//! util.rs - Random utility functions for the program.

use futures::{task::Context, FutureExt};
use std::{future::Future, pin::Pin, task::Poll};
use tokio::{
    runtime::Handle,
    task::{JoinError, JoinHandle},
};

pub fn push_or_else<T, E, F: FnOnce(E)>(res: Result<T, E>, vec: &mut Vec<T>, or_else: F) {
    match res {
        Ok(val) => vec.push(val),
        Err(err) => or_else(err),
    }
}

pub fn poll_join_result<R, F1: FnOnce(R), F2: FnOnce(anyhow::Error)>(
    future: &mut JoinHandle<anyhow::Result<R>>,
    handle: &Handle,
    on_success: F1,
    on_error: F2,
) -> bool {
    let _guard = handle.enter();

    let noop_waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&noop_waker);

    let future = Pin::new(future);

    if let Poll::Ready(res) = future.poll(&mut cx) {
        match res {
            Ok(res) => match res {
                Ok(val) => on_success(val),
                Err(err) => on_error(err),
            },
            Err(err) => on_error(anyhow::Error::from(err)),
        }
        true
    } else {
        false
    }
}

pub fn poll_optional<
    R,
    F1: FnOnce(R),
    F2: FnOnce(anyhow::Error),
    F3: FnOnce() -> Option<JoinHandle<anyhow::Result<R>>>,
>(
    optional_future: &mut Option<JoinHandle<anyhow::Result<R>>>,
    handle: &Handle,
    on_success: F1,
    on_error: F2,
    on_new: F3,
) {
    let mut reset = false;
    if let Some(future) = optional_future {
        reset = poll_join_result(future, handle, on_success, on_error);
    }

    if reset {
        *optional_future = None;
    }

    if optional_future.is_none() {
        if let Some(future) = on_new() {
            *optional_future = Some(future);
        }
    }
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
    pub fn poll_starting<F: FnOnce(anyhow::Error)>(&mut self, handle: &Handle, on_error: F) {
        let mut reset = false;
        let mut inst = None;
        if let RunningState::Starting(f) = self {
            poll_join_result(
                f,
                handle,
                |r| inst = Some(r),
                |e| {
                    reset = true;
                    on_error(e);
                },
            );
        }
        if let Some(inst) = inst {
            *self = RunningState::Running(inst);
        }
        if reset {
            *self = RunningState::NotStarted;
        }
    }
}
