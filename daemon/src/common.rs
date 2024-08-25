use std::collections::HashMap;
use std::io;
use serde::{Deserialize, Serialize};
use surge_ping::SurgeError;
use tokio::process::Child;
use crate::application::Application;
use crate::common::Error::{IoError, PingError};
use crate::email_utils::EmailConfig;
use crate::init::InitializationErrorKind;
use crate::sms_utils::SmsConfig;
use crate::ssh_utils::SshConfig;
use crate::user::User;

#[derive(Debug)]
pub enum Error{
    IoError(io::Error),
    IncomingSmSParsingError,
    SystemCommandExecutionError,
    SshTunnelUrlParsingError,
    ConfigurationParsingError(toml::de::Error),
    QmiResponseParsingError(String),
    SenderNotAllowed(String),
    InvalidRequestError(String),
    DomainNameResolutionError,
    PingError(SurgeError),
    InitialisationFailed(InitializationErrorKind)
}

impl From<io::Error> for Error{
    fn from(value: io::Error) -> Self {
        IoError(value)
    }
}

impl From<SurgeError> for Error{
    fn from(value: SurgeError) -> Self {
        PingError(value)
    }
}

pub type Result<T> = std::result::Result<T,Error>;



#[derive(Debug)]
pub struct Context {
    running_processes: HashMap<u32, Child>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Configuration {
    #[serde(rename = "user")]
    pub users: Vec<User>,
    #[serde(rename = "application")]
    pub applications: Vec<Application>,

    pub sms_config: SmsConfig,
    pub email_config: EmailConfig,
    pub ssh_config: SshConfig
}

impl Context {
    pub fn new() -> Self {
        Self {
            running_processes: HashMap::new(),
        }
    }

    pub fn store_process(&mut self, process: Child) -> u32 {
        let idx = self.running_processes.len() as u32;
        self.running_processes.insert(idx, process);
        idx
    }

    pub fn get_process(&mut self, idx: u32) -> Option<Child> {
        self.running_processes.remove(&idx)
    }
}