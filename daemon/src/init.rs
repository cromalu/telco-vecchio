use std::fs::{create_dir, File};
use std::io::Read;
use std::time::{Duration, SystemTime};
use fern::Output;
use log::{debug, error, info};
use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use serde::{Deserialize, Serialize};
use crate::{common, sms_utils, status};
use crate::common::{Configuration, Context};
use crate::common::Error::ConfigurationParsingError;
use crate::status::DeviceStatus;

const CONFIGURATION_FILE: &str = "/etc/telco-vecchio.conf";
const VAR_DIRECTORY : &str = "/usr/share/telco-vecchio";
const LOG_FILE: &str = "log";
const INIT_LISTENER_REGISTER: &str = "init-listener-register";
const LOG_FILE_MAX_SIZE: u64 = 50000;
const MAX_LOG_FILES: usize = 3;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct InitConfig {
    pub init_status_refresh_period_seconds: u64,
    pub init_status_refresh_max_retry: u32,
}

pub async fn init(is_daemon : bool) -> common::Result<Context> {


    let _ = create_dir(VAR_DIRECTORY);

    let writer: Output = if is_daemon  {
        Output::writer(Box::new(BasicRollingFileAppender::new(
            format!("{}/{}",VAR_DIRECTORY,LOG_FILE),
            RollingConditionBasic::new().max_size(LOG_FILE_MAX_SIZE),
            MAX_LOG_FILES
        ).unwrap()),"\r\n")
    }else{
       Output::stdout("\r\n")
    };

    fern::Dispatch::new()
        .level(log::LevelFilter::Debug)
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339_seconds(SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .chain(writer)
        .apply().unwrap();

    //read config
    let mut configuration_string = String::new();
    File::open(CONFIGURATION_FILE)?.read_to_string(&mut configuration_string)?;
    info!("init - configuration content:\n{}",configuration_string);

    let configuration: Configuration = toml::from_str(&configuration_string).map_err(|e| ConfigurationParsingError(e))?;
    info!("init - configuration read properly");

    //get device current status
    let mut sleep_loop_counter = configuration.init_config.init_status_refresh_max_retry;
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
                    info!("init - device not yet connected to network, retrying after {} seconds",configuration.init_config.init_status_refresh_period_seconds);
                    sleep_loop_counter = sleep_loop_counter - 1;
                    tokio::time::sleep(Duration::from_secs(configuration.init_config.init_status_refresh_period_seconds)).await;
                    status = status::get_status(&configuration).await?;
                } else {
                    error!("init - device still not connected to network after retries");
                    break;
                }
            }
            DeviceStatus::Ready => {
                info!("init - device is ready");
                sms_utils::init(&configuration.sms_config).await?;
                break;
            }
        }
    };
    let context = Context::new(configuration, status);
    info!("init - initialization done");
    Ok(context)
}


