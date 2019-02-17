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
        "TODO(indutny): implement me"
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NotRunning => write!(f, "run() must be called first"),
            Error::MPSCRecv => write!(f, "MPSCRecv"),
            Error::MPSCTrySend => write!(f, "MPSCTrySend"),
            Error::OneShotRecv => write!(f, "OneShotRecv"),
            Error::OneShotSend => write!(f, "OneShotSend"),
        }
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
