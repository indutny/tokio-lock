//! # tokio-lock
//!
//! Access an object from a single Tokio task.
//!
//!
//! Usage:
//! ```rust
//! extern crate futures;
//! extern crate tokio;
//! extern crate tokio_lock;
//!
//! use futures::prelude::*;
//! use futures::future::{self, FutureResult};
//!
//! use tokio_lock::{Lock, Error};
//!
//! // Create a Lock instance
//! let mut lock = Lock::new();
//!
//! struct TestObject {
//!   field: u32,
//! }
//!
//! // Create a future that is going to manage the `TestObject` instance.
//! // NOTE: Object is consumed in the process.
//! let manage = lock.manage(TestObject { field: 42 });
//!
//! // Borrow an object from `lock` and execute given closure.
//! let get_field = lock.get(|obj| -> FutureResult<u32, Error> {
//!     future::ok(obj.field)
//! }).map(move |field| {
//!     assert_eq!(field, 42);
//!
//!     // Stop managing the object
//!     // NOTE: This may not be needed in the most of the cases.
//!     lock.stop();
//! });
//!
//! // NOTE: `manage` is a future and has to be run
//! tokio::run(manage.join(get_field).map_err(|err| {
//!     panic!("Got error");
//! }).map(|_| ()));
//! ```

extern crate futures;
extern crate tokio;

mod error;

use futures::prelude::*;
use futures::{future, Future};
use std::any::Any;
use std::error::Error as StdError;
use tokio::sync::{mpsc, oneshot};

/// Possible error values
pub use error::Error;

type AnyBox = Box<Any + Send + 'static>;

/// This structure "locks" an object to be accessed from a single Tokio task.
pub struct Lock<T, E>
where
    E: StdError + From<Error> + Send + 'static,
{
    tx: Option<mpsc::UnboundedSender<Acquire<T, E>>>,
}

enum Closure<T, E>
where
    E: StdError + From<Error> + Send + 'static,
{
    Read(Box<(FnMut(&T) -> Box<Future<Item = AnyBox, Error = E> + Send>) + Send>),
    Write(Box<(FnMut(&mut T) -> Box<Future<Item = AnyBox, Error = E> + Send>) + Send>),
}

struct Acquire<T, E>
where
    E: StdError + From<Error> + Send + 'static,
{
    tx: oneshot::Sender<Result<AnyBox, E>>,
    closure: Closure<T, E>,
}

impl<T, E> Lock<T, E>
where
    E: StdError + From<Error> + Send + 'static,
{
    /// Create new instance of a `Lock`.
    pub fn new() -> Self {
        Self { tx: None }
    }

    /// Consume `value` and return a `Future` for managing it.
    pub fn manage(&mut self, mut value: T) -> impl Future<Item = (), Error = Error> {
        let (tx, rx) = mpsc::unbounded_channel();

        self.tx = Some(tx);

        rx.from_err::<Error>()
            .for_each(move |acquire| {
                let (res_tx, closure) = (acquire.tx, acquire.closure);
                let item = match closure {
                    Closure::Read(mut f) => f(&value),
                    Closure::Write(mut f) => f(&mut value),
                };

                item.then(move |res| res_tx.send(res).map_err(|_| Error::OneShotSend))
                    .from_err()
            })
            .from_err()
    }

    fn run_closure(
        &mut self,
        closure: Closure<T, E>,
    ) -> Box<Future<Item = AnyBox, Error = E> + Send> {
        let tx = match &mut self.tx {
            Some(tx) => tx,
            None => {
                return Box::new(future::err(E::from(Error::NotRunning)));
            }
        };

        let (res_tx, res_rx) = oneshot::channel();

        let acquire = Acquire {
            tx: res_tx,
            closure,
        };
        if let Err(err) = tx.try_send(acquire) {
            return Box::new(future::err(E::from(Error::from(err))));
        }

        Box::new(res_rx.from_err::<Error>().from_err().and_then(|res| res))
    }

    /// Get the managed object and invoke `cb` with a reference to it.
    pub fn get<CB, F, I>(&mut self, mut cb: CB) -> impl Future<Item = I, Error = E>
    where
        CB: (FnMut(&T) -> F) + Send + 'static,
        F: Future<Item = I, Error = E> + Send + 'static,
        I: Send + 'static,
    {
        let closure = Closure::Read(Box::new(move |t| {
            Box::new(cb(t).map(|t| -> AnyBox { Box::new(t) }))
        }));
        self.run_closure(closure)
            .map(|res| -> I { *res.downcast::<I>().unwrap() })
    }

    /// Get the managed object and invoke `cb` with a mutable reference to it.
    pub fn get_mut<I, CB, F>(&mut self, mut cb: CB) -> impl Future<Item = I, Error = E>
    where
        CB: (FnMut(&mut T) -> F) + Send + 'static,
        F: Future<Item = I, Error = E> + Send + 'static,
        I: Send + 'static,
    {
        let closure = Closure::Write(Box::new(move |t| {
            Box::new(cb(t).map(|t| -> AnyBox { Box::new(t) }))
        }));
        self.run_closure(closure)
            .map(|res| -> I { *res.downcast::<I>().unwrap() })
    }

    /// Stop managing the object.
    pub fn stop(&mut self) {
        self.tx = None;
    }
}

impl<T, E> Default for Lock<T, E>
where
    E: StdError + From<Error> + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, E> Clone for Lock<T, E>
where
    E: StdError + From<Error> + Send + 'static,
{
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::FutureResult;

    struct TestObject {
        x: u32,
        y: u64,
    }

    #[test]
    fn it_should_compute_digest_for_abc() {
        let o = TestObject { x: 23, y: 42 };

        let mut l = Lock::new();
        let poll = l.manage(o).map_err(|err| {
            panic!("Got error {}", err);
        });

        let get_x = l.get(|o| -> FutureResult<u32, Error> { future::ok(o.x) });
        let get_y = l
            .clone()
            .get(|o| -> FutureResult<u64, Error> { future::ok(o.y) });

        let get = get_x
            .join(get_y)
            .map_err(|err| {
                panic!("Got error {}", err);
            })
            .map(move |val| {
                assert_eq!(val, (23, 42));
                l.stop();
            });

        tokio::run(poll.join(get).map(|_| ()));
    }
}
