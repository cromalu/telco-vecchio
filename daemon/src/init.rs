use std::fs::{create_dir, File, remove_file};
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};
use fern::Output;
use log::{debug, error, info};
use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use serde::{Deserialize, Serialize};
use crate::{common, sms_utils, status};
use crate::common::{Configuration, Context};
use crate::common::Error::ConfigurationParsingError;
use crate::sms_utils::OutgoingSms;
use crate::status::DeviceStatus;
use crate::user::User;

const CONFIGURATION_FILE: &str = "/etc/telco-vecchio.conf";
const SHARE_DIRECTORY: &str = "/usr/share/telco-vecchio";
const LOG_DIRECTORY: &str = "/tmp/log/telco-vecchio";
const LOG_FILE: &str = "log";
const INIT_LISTENER_REGISTER: &str = "init-listener-register";

const LOG_FILE_MAX_SIZE: u64 = 500000;
const MAX_LOG_FILES: usize = 2;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct InitConfig {
    pub init_status_refresh_period_seconds: u64,
    pub init_status_refresh_max_retry: u32,
}

pub async fn init(is_daemon : bool) -> common::Result<Context> {

    let _ = create_dir(LOG_DIRECTORY);
    let _ = create_dir(SHARE_DIRECTORY);

    let writer: Output = if is_daemon  {
        Output::writer(Box::new(BasicRollingFileAppender::new(
            format!("{}/{}", LOG_DIRECTORY, LOG_FILE),
            RollingConditionBasic::new().max_size(LOG_FILE_MAX_SIZE),
            MAX_LOG_FILES - 1
        ).unwrap()),"\r\n")
    }else{
       Output::stdout("\r\n")
    };

    fern::Dispatch::new()
        .level(log::LevelFilter::Info)
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
                break;
            }
        }
    };

    if status.device_status != DeviceStatus::SimLocked && status.device_status != DeviceStatus::LteNotConnected {
        //even if status is not ready, sms might be sent or received
        sms_utils::init(&configuration.sms_config).await?;


        if let Some(init_listener) = lookup_init_listener(&configuration){
            info!("init - notifying registered init listener : {}",init_listener.name);
            sms_utils::send_sms(&configuration.sms_config, &OutgoingSms {
                to: init_listener.phone_number.to_string(),
                msg: format!("Telco-Vecchio is up.\n{}",status.to_string())
            }).await.unwrap_or_else(|e|{
                error!("init - cannot notify registered init listener - error : {:?}",e);
            })
        }
    };

    let context = Context::new(configuration, status);
    info!("init - initialization success");
    Ok(context)
}

pub fn register_init_listener(user: &User){
    let path = format!("{}/{}", SHARE_DIRECTORY, INIT_LISTENER_REGISTER);
    //erase any previous content in the file
    match std::fs::OpenOptions::new().create(true).write(true).truncate(true).open(Path::new(&path)) {
        Ok(mut file) => {
            match file.write(user.name.as_bytes()) {
                Ok(_) => {
                    debug!("register_init_listener - user {:?} registered as init listener", user.name)
                }
                Err(e) => {
                    error!("register_init_listener - cannot write to register file {:?} - error: {:?}",&path,e);
                }
            }
        }
        Err(e) => {
            error!("register_init_listener - cannot open register file {:?} - error: {:?}",&path,e);
        }
    }
}

fn lookup_init_listener(configuration: &Configuration)-> Option<&User>{
    let path = format!("{}/{}", SHARE_DIRECTORY, INIT_LISTENER_REGISTER);
    if !Path::exists(Path::new(&path)){
        debug!("lookup_init_listener - register file {} does not exists",&path);
        return None
    }

    let user = match File::open(&path) {
        Ok(mut file) => {
            debug!("lookup_init_listener - register file");
            let mut content = String::new();
            match file.read_to_string(&mut content) {
                Ok(_) => {
                    debug!("lookup_init_listener - register file content: {:?}",content);
                    configuration.users.iter().find(|user| {user.name == content})
                }
                Err(e) => {
                    error!("lookup_init_listener - cannot read register file {:?} - error: {:?}",path,e);
                    None
                }
            }
        }
        Err(e) => {
            error!("lookup_init_listener - cannot open register file {:?} - error: {:?}",&path,e);
            None
        }
    };
    remove_file(Path::new(&path)).unwrap_or_else(|e|{
        error!("lookup_init_listener - cannot delete register file {:?} - error: {:?}",&path,e);
    });
    user
}



