use std::io;
use crate::common::Error::{IoError, SerialPortError};

#[derive(Debug)]
pub enum Error{
    IoError(std::io::Error),
    SerialPortError(serialport::Error)
}

impl From<io::Error> for Error{
    fn from(value: io::Error) -> Self {
        IoError(value)
    }
}

impl From<serialport::Error> for Error{
    fn from(value: serialport::Error) -> Self {
        SerialPortError(value)
    }
}

pub type Result<T> = std::result::Result<T,Error>;