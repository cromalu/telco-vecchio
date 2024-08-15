use std::io;
use crate::common::Error::IoError;

#[derive(Debug)]
pub enum Error{
    IoError(io::Error),
    IncomingSmSParsingError,
    SshTunnelUrlParsingError,
    SystemCommandExecutionError,
    ConfigurationParsingError(serde_yaml::Error),
    SenderNotAllowed(String),
    InvalidRequestError(String)
}

impl From<io::Error> for Error{
    fn from(value: io::Error) -> Self {
        IoError(value)
    }
}


pub type Result<T> = std::result::Result<T,Error>;