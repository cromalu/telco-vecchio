use std::fs::File;
use std::io::Read;
use std::time::Duration;
use log::{error, info};
use crate::{common, status};
use crate::common::Configuration;
use crate::common::Error::ConfigurationParsingError;
use crate::init::InitializationErrorKind::{CannotUnlockSimCard, EmailServiceNotReachable, NetworkUnreachable, NoApplicationAvailable, SshTunnelServiceNotReachable};
use crate::status::{DeviceStatus, ServiceStatus};

const CONFIGURATION_FILE: &str = "/etc/telco-vecchio.conf";
const INITIAL_STATUS_RESOLUTION_POLLING_PERIOD_SECS: u64 = 10;

pub async fn init() -> common::Result<Configuration> {
    info!("init - starting");

    //read config
    let mut configuration_string = String::new();
    File::open(CONFIGURATION_FILE)?.read_to_string(&mut configuration_string)?;
    let mut configuration: Configuration = toml::from_str(&configuration_string).map_err(|e| ConfigurationParsingError(e))?;
    info!("init - configuration read properly");

    //get device current status
    let mut sleep_loop_counter = 12;
    let mut sim_unlock_performed = false;
    let mut status = status::get_status(&configuration).await?;
    let init_error = loop {
        match status.device_status {
            DeviceStatus::SimLocked => {
                if !sim_unlock_performed {
                    info!("init - sim card is locked, unlocking it");
                    //todo try pin and retry init once
                    sim_unlock_performed = true;
                } else {
                    error!("init - sim card is still locked after unlocking attempt, configured pin might be wrong");
                    break Some(CannotUnlockSimCard);
                }
            }
            DeviceStatus::LteNotConnected | DeviceStatus::InternetUnreachable => {
                //wait for a while and retry
                if sleep_loop_counter > 0 {
                    info!("init - device not yet connected to network, retrying after {} seconds",INITIAL_STATUS_RESOLUTION_POLLING_PERIOD_SECS);
                    sleep_loop_counter = sleep_loop_counter - 1;
                    tokio::time::sleep(Duration::from_secs(INITIAL_STATUS_RESOLUTION_POLLING_PERIOD_SECS)).await;
                    status = status::get_status(&configuration).await?;
                } else {
                    error!("init - device still not connected to network after retries");
                    break Some(NetworkUnreachable);
                }
            }
            DeviceStatus::Ready => {
                info!("init - device is ready");
                break None;
            }
        }
    };

    if let Some(error_kind) = init_error {
        error!("init - device cannot be ready: {:?} - initialization failed",error_kind);
        return Err(common::Error::InitialisationFailed(error_kind));
    }

    if let ServiceStatus::Unreachable = status.email_service_status{
        error!("init - email service is not reachable - initialization failed");
        return Err(common::Error::InitialisationFailed(EmailServiceNotReachable));
    }

    if let ServiceStatus::Unreachable = status.ssh_tunnel_service_status{
        error!("init - ssh tunnel service is not reachable - initialization failed");
        return Err(common::Error::InitialisationFailed(SshTunnelServiceNotReachable));
    }

    //adjusting configuration by keeping only the available applications
    configuration.applications = configuration.applications.into_iter().filter(|app| { status.applications_status[&app.name] == ServiceStatus::Reachable }).collect();
    if configuration.applications.is_empty(){
        error!("init - no application available - initialization failed");
        return Err(common::Error::InitialisationFailed(NoApplicationAvailable));
    }

    info!("init - initialization success");
    Ok(configuration)
}

#[derive(Debug)]
pub enum InitializationErrorKind{
    CannotUnlockSimCard,
    NetworkUnreachable,
    EmailServiceNotReachable,
    SshTunnelServiceNotReachable,
    NoApplicationAvailable
}

