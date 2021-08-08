use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("generic: {0}")]
    Generic(String),

    #[error("malformed message: {0}")]
    MalformedMessage(String),

    #[error("io: {0}")]
    Io(#[from] io::Error),

    #[error("DNS: {0}")]
    DnsError(String),

    #[error("stream finished")]
    Finished,
}
