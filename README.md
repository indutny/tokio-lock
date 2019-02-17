# tokio-lock
[![Build Status](https://secure.travis-ci.org/indutny/tokio-lock.svg)](http://travis-ci.org/indutny/tokio-lock)
[![Latest version](https://img.shields.io/crates/v/tokio-lock.svg)](https://crates.io/crates/tokio-lock)
[![Documentation](https://docs.rs/tokio-lock/badge.svg)][docs]
![License](https://img.shields.io/crates/l/tokio-lock.svg)

Access an object from a single [Tokio][tokio] task.

## Why?

[Tokio][tokio] futures run in a multi-threaded environment, which makes
accessing an object from different futures complicated. In many cases, however,
the object is not thread safe, and has to be accessed from a single thread.

As an example, imagine writing HTTP wrapper for an RPC server. Most likely, this
has to be done either with a MPSC (multiple producer single consumer) queue and
a large enum of possible messages, or with a mutex guarding access to the
object.

This library creates a convenient abstraction on top of MPSC, that is managed
internally.

## Quick example

```rust
extern crate futures;
extern crate tokio;
extern crate tokio_lock;

use futures::prelude::*;
use futures::future::{self, FutureResult};

use tokio_lock::{Lock, Error};

// Create a Lock instance
let mut lock = Lock::new();

struct TestObject {
    field: u32,
}

// Create a future that is going to manage the `TestObject` instance.
// NOTE: Object is consumed in the process.
let manage = lock.manage(TestObject { field: 42 });

// Borrow an object from `lock` and execute given closure.
let get_field = lock.get(|obj| -> FutureResult<u32, Error> {
    future::ok(obj.field)
}).map(move |field| {
    assert_eq!(field, 42);

    // Stop managing the object
    // NOTE: This may not be needed in the most of the cases.
    lock.stop();
});

// NOTE: `manage` is a future and has to be run
tokio::run(manage.join(get_field).map_err(|err| {
    panic!("Got error");
}).map(|_| ()));
```

## Using tokio-lock

Please check our [documentation][docs] for details.

[tokio]: https://tokio.rs/
[docs]: https://docs.rs/tokio-lock
