use std::collections::HashMap;
use std::io;
use std::time::{Duration, SystemTime};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use surge_ping::SurgeError;
use tokio::process::Child;
use crate::application::Application;
use crate::common::Error::{IoError, PingError};
use crate::email_utils::EmailConfig;
use crate::init::InitConfig;
use crate::sms_utils;
use crate::sms_utils::{OutgoingSms, SmsConfig};
use crate::ssh_utils::SshConfig;
use crate::status::Status;
use crate::user::User;

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    SmsInitError,
    SmsReadingError,
    SmsSendingError,
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
    InvalidStatus(String),
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        IoError(value)
    }
}

impl From<SurgeError> for Error {
    fn from(value: SurgeError) -> Self {
        PingError(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;


pub struct Context {
    pub configuration: Configuration,
    pub status: Status,
    pub tunnels: HashMap<u32, Tunnel>,
}


pub struct Tunnel {
    pub user: String,
    pub application: String,
    pub process: Child,
    pub creation_date: SystemTime,
}

impl Tunnel {
    pub fn new(user: String, application: String, process: Child) -> Self {
        Self { user, application, process, creation_date: SystemTime::now() }
    }
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
}

impl Context {
    pub fn new(configuration: Configuration, status: Status) -> Self {
        Self {
            configuration,
            status,
            tunnels: HashMap::new(),
        }
    }

    pub fn update_status(&mut self, status: Status) {
        self.status = status;
    }


    pub async fn clean_up_expired_tunnels(&mut self) {
        debug!("clean_up_expired_tunnels: start");
        let current_time = SystemTime::now();
        let max_duration = Duration::from_secs(self.configuration.ssh_config.tunnel_timeout_sec);

        let mut id_to_remove = Vec::new();
        for (id, tunnel) in &mut self.tunnels {
            if current_time.duration_since(tunnel.creation_date).unwrap() > max_duration {
                info!("clean_up_expired_tunnels: tunnel: {} has expired",id);

                //killing process
                if let Err(e) = tunnel.process.kill().await {
                    error!("clean_up_expired_tunnels: cannot kill process - error: {:?}",e);
                }

                //notifying user
                if let Some(user) = self.configuration.users.iter().find(|u| { u.name == tunnel.user }) {
                    debug!("clean_up_expired_tunnels - notifying user: {} about expiration",user.name);
                    sms_utils::send_sms(&self.configuration.sms_config, &OutgoingSms {
                        to: user.phone_number.to_string(),
                        msg: format!("Expired tunnel {} has been closed", id),
                    }).await.unwrap_or_else(|e| {
                        error!("clean_up_expired_tunnels - cannot notify user - error : {:?}",e);
                    })
                } else {
                    error!("clean_up_expired_tunnels: user {} not found",tunnel.user);
                }

                //retuning tunnel id to remove from map
                id_to_remove.push(id.clone());
            }
        }
        id_to_remove.iter().for_each(|id| {
            let _ = self.tunnels.remove(id);
        });
        debug!("clean_up_expired_tunnels: done");
    }
}