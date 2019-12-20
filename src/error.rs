use std::io;

#[derive(Debug)]
pub enum Error {
    Error(String),
    MalformedMessage(String),
    Io(io::Error),
    Finished
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}
