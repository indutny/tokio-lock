extern crate futures;
extern crate tokio;

mod error;

use futures::prelude::*;
use futures::{future, Future};
use std::any::Any;
use std::error::Error as StdError;
use tokio::sync::{mpsc, oneshot};

pub use error::Error;

type AnyBox = Box<Any + Send + 'static>;

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
    pub fn new() -> Self {
        Self { tx: None }
    }

    pub fn run(&mut self, mut value: T) -> impl Future<Item = (), Error = Error> {
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
        let poll = l.run(o).map_err(|err| {
            panic!("Got error {}", err);
        });

        let get_x = l.get(|o| -> FutureResult<u32, Error> { future::ok(o.x) });
        let get_y = l.get(|o| -> FutureResult<u64, Error> { future::ok(o.y) });

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
