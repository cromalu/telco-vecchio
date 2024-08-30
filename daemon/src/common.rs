use std::collections::HashMap;
use std::io;
use serde::{Deserialize, Serialize};
use surge_ping::SurgeError;
use tokio::process::Child;
use crate::application::Application;
use crate::common::Error::{IoError, PingError};
use crate::email_utils::EmailConfig;
use crate::init::InitConfig;
use crate::sms_utils::SmsConfig;
use crate::ssh_utils::SshConfig;
use crate::status::{InvalidStatusKind, MonitoringConfig, Status};
use crate::user::User;

#[derive(Debug)]
pub enum Error{
    IoError(io::Error),
    IncomingSmSParsingError,
    SmsProcessingError(String),
    SystemCommandExecutionError,
    SshTunnelUrlParsingError,
    SshTunnelUrlSetupTimeout,
    SshTunnelServiceError(String),
    ConfigurationParsingError(toml::de::Error),
    QmiResponseParsingError(String),
    SenderNotAllowed(String),
    InvalidRequestError(String),
    DomainNameResolutionError,
    PingError(SurgeError),
    InvalidStatus(InvalidStatusKind)
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
    pub configuration: Configuration,
    pub status: Status,
    pub running_processes: HashMap<u32, (String,Child)>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Configuration {
    #[serde(rename = "user")]
    pub users: Vec<User>,
    #[serde(rename = "application")]
    pub applications: Vec<Application>,

    pub sms_config: SmsConfig,
    pub email_config: EmailConfig,
    pub ssh_config: SshConfig,

    pub init_config: InitConfig,
    pub monitoring_config: MonitoringConfig,
}

impl Context {
    pub fn new(configuration: Configuration, status: Status) -> Self {
        Self {
            configuration,
            status,
            running_processes: HashMap::new(),
        }
    }

    pub fn update_status(&mut self, status: Status){
        self.status = status;
    }
}