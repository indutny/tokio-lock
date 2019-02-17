extern crate futures;
extern crate tokio;

mod error;

use futures::prelude::*;
use futures::{future, Future};
use std::error::Error as StdError;
use tokio::sync::{mpsc, oneshot};

pub use error::Error;

pub struct Lock<T, I, E>
where
    I: Send + 'static,
    E: StdError + From<Error> + Send + 'static,
{
    tx: Option<mpsc::UnboundedSender<Acquire<T, I, E>>>,
}

enum Closure<T, I, E>
where
    I: Send + 'static,
    E: StdError + From<Error> + Send + 'static,
{
    Read(Box<(FnMut(&T) -> Box<Future<Item = I, Error = E> + Send>) + Send>),
    Write(Box<(FnMut(&mut T) -> Box<Future<Item = I, Error = E> + Send>) + Send>),
}

struct Acquire<T, I, E>
where
    I: Send + 'static,
    E: StdError + From<Error> + Send + 'static,
{
    tx: oneshot::Sender<Result<I, E>>,
    closure: Closure<T, I, E>,
}

impl<T, I, E> Lock<T, I, E>
where
    I: Send + 'static,
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
        closure: Closure<T, I, E>,
    ) -> Box<Future<Item = I, Error = E> + Send> {
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

    pub fn get<F>(
        &mut self,
        f: F,
    ) -> Box<Future<Item = I, Error = E> + Send>
        where
        F: (FnMut(&T) -> Box<Future<Item = I, Error = E> + Send>) + Send + 'static,
    {
        self.run_closure(Closure::Read(Box::new(f)))
    }

    pub fn get_mut<F>(
        &mut self,
        f: F
    ) -> Box<Future<Item = I, Error = E> + Send>
        where
        F: (FnMut(&mut T) -> Box<Future<Item = I, Error = E> + Send>) + Send + 'static,
    {
        self.run_closure(Closure::Write(Box::new(f)))
    }

    pub fn stop(&mut self) {
        self.tx = None;
    }
}

impl<T, I, E> Default for Lock<T, I, E>
where
    I: Send + 'static,
    E: StdError + From<Error> + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestObject {
        x: u32,
    }

    #[test]
    fn it_should_compute_digest_for_abc() {
        let o = TestObject { x: 23 };

        let mut l = Lock::new();
        let poll = l.run(o).map_err(|err| {
            panic!("Got error {}", err);
        });

        let get = l
            .get(|o: &TestObject| -> Box<Future<Item = u32, Error = Error> + Send> {
                Box::new(future::ok(o.x))
            })
            .map_err(|err| {
                panic!("Got error {}", err);
            })
            .map(move |val| {
                assert_eq!(val, 23);
                l.stop();
            });

        tokio::run(poll.join(get).map(|_| ()));
    }
}
