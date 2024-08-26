use std::fs::File;
use std::io::Read;
use std::time::Duration;
use log::{error, info};
use crate::{common, status};
use crate::common::{Configuration, Context};
use crate::common::Error::ConfigurationParsingError;
use crate::status::DeviceStatus;

const CONFIGURATION_FILE: &str = "/etc/telco-vecchio.conf";
const INITIAL_STATUS_RESOLUTION_POLLING_PERIOD_SECS: u64 = 10;

pub async fn init() -> common::Result<Context> {
    info!("init - starting");

    //read config
    let mut configuration_string = String::new();
    File::open(CONFIGURATION_FILE)?.read_to_string(&mut configuration_string)?;
    info!("init - configuration content:\n{}",configuration_string);

    let configuration: Configuration = toml::from_str(&configuration_string).map_err(|e| ConfigurationParsingError(e))?;
    info!("init - configuration read properly");

    //get device current status
    let mut sleep_loop_counter = 12;
    let mut sim_unlock_performed = false;
    let mut status = status::get_status(&configuration).await?;

    //tempo loop to make sure a reliable status has been resolved
    loop {
        match status.device_status {
            DeviceStatus::SimLocked => {
                if !sim_unlock_performed {
                    info!("init - sim card is locked, unlocking it");
                    //todo try pin
                    status = status::get_status(&configuration).await?;
                    sim_unlock_performed = true;
                } else {
                    error!("init - sim card is still locked after unlocking attempt, configured pin might be wrong");
                    break;
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
                    break;
                }
            }
            DeviceStatus::Ready => {
                info!("init - device is ready");
                break;
            }
        }
    };
    let context = Context::new(configuration, status);
    info!("init - initialization done");
    Ok(context)
}


