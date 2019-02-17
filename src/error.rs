extern crate tokio;

use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NotRunning,
    MPSCRecv,
    MPSCTrySend,
    OneShotRecv,
    OneShotSend,
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::NotRunning => "run() must be called first",
            Error::MPSCRecv => "mpsc recieve failed",
            Error::MPSCTrySend => "mpsc `try_send()` failed",
            Error::OneShotRecv => "oneshot receive failed",
            Error::OneShotSend => "oneshot `send()` failed",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl From<tokio::sync::mpsc::error::UnboundedRecvError> for Error {
    fn from(_: tokio::sync::mpsc::error::UnboundedRecvError) -> Self {
        Error::MPSCRecv
    }
}

impl<T> From<tokio::sync::mpsc::error::UnboundedTrySendError<T>> for Error {
    fn from(_: tokio::sync::mpsc::error::UnboundedTrySendError<T>) -> Self {
        Error::MPSCTrySend
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for Error {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        Error::OneShotRecv
    }
}
