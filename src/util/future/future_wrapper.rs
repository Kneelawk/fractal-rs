//! This contains the `FutureWrapper` and associated objects.

#![allow(dead_code)]

use crate::util::future::poll_unpin;
use std::{future::Future, task::Poll};
use tokio::{
    runtime::Handle,
    task::{JoinError, JoinHandle},
};

/// Describes a `Future` that can be polled.
///
/// This can have a `Future` in it or it can be empty. If it is empty, then
/// nothing happens when polled. If it has a future in it, then it is polled
/// every time a `poll` method is called.
pub struct FutureWrapper<F: Future> {
    future: Option<F>,
}

impl<F: Future> Default for FutureWrapper<F> {
    fn default() -> Self {
        Self { future: None }
    }
}

impl<F: Future> FutureWrapper<F> {
    /// Initializes this wrapper with a future already in it.
    pub fn from_future(future: F) -> FutureWrapper<F> {
        FutureWrapper {
            future: Some(future),
        }
    }

    /// Puts a future into this future wrapper if one is not already in it.
    pub fn insert(&mut self, future: F) -> Result<(), InsertError> {
        if self.future.is_some() {
            return Err(InsertError::NotEmpty);
        }

        self.future = Some(future);

        Ok(())
    }

    /// Returns whether this wrapper contains a future.
    pub fn contains_future(&self) -> bool {
        self.future.is_some()
    }

    /// Returns true if this wrapper does not contain a future.
    pub fn is_empty(&self) -> bool {
        self.future.is_none()
    }
}

impl<F: Future + Unpin> FutureWrapper<F> {
    /// Polls this wrapper's current future. Returns `Some(..)` if this wrapper
    /// contained a future and the future completed.
    pub fn poll_unpin(&mut self, handle: &Handle) -> Option<F::Output> {
        if self.future.is_some() {
            if let Poll::Ready(output) = poll_unpin(handle, self.future.as_mut().unwrap()) {
                self.future = None;
                Some(output)
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl<O: Send + 'static> FutureWrapper<JoinHandle<O>> {
    /// Spawns a future on the runtime referenced by `handle` and initializes
    /// this wrapper with the resulting `JoinHandle`.
    pub fn from_spawn<F: Future<Output = O> + Send + 'static>(
        handle: &Handle,
        future: F,
    ) -> FutureWrapper<JoinHandle<O>> {
        FutureWrapper {
            future: Some(handle.spawn(future)),
        }
    }

    /// Spawns a future on the runtime referenced by `handle` and inserts the
    /// resulting `JoinHandle` into self.
    pub fn insert_spawn<F: Future<Output = O> + Send + 'static>(
        &mut self,
        handle: &Handle,
        future: F,
    ) -> Result<(), InsertError> {
        if self.future.is_some() {
            return Err(InsertError::NotEmpty);
        }

        self.future = Some(handle.spawn(future));

        Ok(())
    }
}

impl<T, E: From<JoinError>> FutureWrapper<JoinHandle<Result<T, E>>> {
    /// Polls a `Future` that is a `JoinHandle` who's output is a `Result` who's
    /// error can be converted from a `JoinError`.
    pub fn poll_join_result(&mut self, handle: &Handle) -> Option<Result<T, E>> {
        if self.future.is_some() {
            if let Poll::Ready(res) = poll_unpin(handle, self.future.as_mut().unwrap()) {
                Some(match res {
                    Ok(res) => res,
                    Err(err) => Err(E::from(err)),
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Error)]
pub enum InsertError {
    #[error("This future wrapper is not empty")]
    NotEmpty,
}
