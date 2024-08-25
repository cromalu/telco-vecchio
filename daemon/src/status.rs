use std::collections::HashMap;
use std::net::IpAddr;
use std::process::Stdio;
use log::{debug, error, info};
use tinyjson::JsonValue;
use tokio::process::Command;
use crate::common;
use crate::common::Configuration;
use crate::common::Error::QmiResponseParsingError;
use crate::status::ServiceStatus::{Reachable, Unreachable};

const QMI_BINARY: &str = "uqmi";
const QMI_DEVICE: &str = "/dev/cdc-wdm0";
const PINGABLE_HOST: &str = "8.8.8.8";

pub struct Status {
    pub device_status: DeviceStatus,
    pub email_service_status: ServiceStatus,
    pub ssh_tunnel_service_status: ServiceStatus,
    pub applications_status: HashMap<String, ServiceStatus>,
}

#[derive(Debug,PartialEq)]
pub enum ServiceStatus {
    Reachable,
    Unreachable,
}

#[derive(Debug)]
pub enum DeviceStatus {
    SimLocked,
    LteNotConnected,
    InternetUnreachable,
    Ready,
}

pub async fn get_status(configuration: &Configuration) -> common::Result<Status> {

    //device internal status
    let device_status = get_device_status().await?;
    info!("get_status: device status: {:?}",device_status);

    //get external services status
    let (email_service_status, ssh_tunnel_service_status) =
        if let DeviceStatus::Ready = device_status {
            (
                //email
                if let Ok(()) = ping_domain(&configuration.email_config.server_domain).await {
                    Reachable
                } else {
                    error!("get_status - email service unreachable");
                    Unreachable
                },
                //ssh tunnel
                if let Ok(()) = ping_domain(&configuration.ssh_config.service_host).await {
                    Reachable
                } else {
                    error!("get_status - ssh tunnel service unreachable");
                    Unreachable
                },
            )
        } else {
            (Unreachable, Unreachable)
        };
    info!("get_status: email service status: {:?} - ssh tunnel service status: {:?}",email_service_status,ssh_tunnel_service_status);

    //local application status
    let mut applications_status = HashMap::new();
    for application in &configuration.applications {
        let status = match surge_ping::ping(application.host_ip, &[0; 8]).await {
            Ok((_, duration)) => {
                debug!("get_status - {} ping ok - duration: {:?}",application.name, duration);
                Reachable
            }
            Err(e) => {
                error!("get_status - cannot ping {}: {:?}",application.name ,e);
                Unreachable
            }
        };
        info!("get_status: application: {} - status: {:?}",application.name,status);
        let _ = applications_status.insert(application.name.clone(), status);
    }

    Ok(Status{
        device_status,
        email_service_status,
        ssh_tunnel_service_status,
        applications_status,
    })
}



async fn ping_domain(domain: &String) -> common::Result<()> {
    debug!("ping_domain: pinging {} ...", domain);
    let dns_result = dns_lookup::lookup_host(&domain)?;
    let ip = dns_result.into_iter().next().ok_or(common::Error::DomainNameResolutionError)?;
    debug!("ping_domain: server ip address resolved {:?}", ip);
    let (_, duration) = surge_ping::ping(ip,&[0; 8]).await?;
    debug!("ping_domain: domain ping ok - duration: {:?}",duration);
    Ok(())
}


async fn get_device_status() -> common::Result<DeviceStatus> {
    info!("get_device_status - starting");
    if is_sim_locked().await? {
        return Ok(DeviceStatus::SimLocked);
    }
    info!("get_device_status - sim card unlocked");
    if !is_connected_to_lte().await? {
        return Ok(DeviceStatus::LteNotConnected);
    }
    info!("get_device_status - device connected to lte");
    //check internet access
    if !is_connected_to_internet().await {
        return Ok(DeviceStatus::InternetUnreachable);
    }
    info!("get_device_status - device connected to internet");
    Ok(DeviceStatus::Ready)
}


async fn is_connected_to_internet() -> bool {
    match surge_ping::ping(IpAddr::V4(PINGABLE_HOST.parse().unwrap()), &[0; 8]).await {
        Ok((_, duration)) => {
            debug!("is_connected_to_internet - internet ping ok - duration: {:?}",duration);
            true
        }
        Err(e) => {
            error!("is_connected_to_internet - cannot ping internet: {:?}",e);
            false
        }
    }
}

async fn is_connected_to_lte() -> common::Result<bool> {
    let system_info_string = qmi_command("--get-system-info").await?;
    let system_info_json: JsonValue = system_info_string.parse().map_err(|_| { QmiResponseParsingError("cannot parse --get-system-info response into json".to_string()) })?;
    let service_status: &String = system_info_json["lte"]["service_status"].get().ok_or(QmiResponseParsingError("cannot read lte service status from system info".to_string()))?;
    debug!("is_connected_to_lte - service status: {}",service_status);
    let is_connected = match service_status.as_str() {
        "available" => {
            true
        }
        _ => {
            false
        }
    };
    Ok(is_connected)
}


//todo fn unlock_sim_card() {}

async fn is_sim_locked() -> common::Result<bool> {
    let sim_state_string = qmi_command("--uim-get-sim-state").await?;
    let sim_state_json: JsonValue = sim_state_string.parse().map_err(|_| { QmiResponseParsingError("cannot parse --uim-get-sim-state response into json".to_string()) })?;
    let pin_status: &String = sim_state_json["pin1_status"].get().ok_or(QmiResponseParsingError("cannot read pin1_status from sim state info".to_string()))?;
    debug!("is_sim_locked - pin status: {}",pin_status);
    let is_locked = match pin_status.as_str() {
        "disabled" => {
            debug!("is_sim_locked - no pin set");
            false
        }
        //to do
        _ => false
    };
    Ok(is_locked)
}


///Qualcomm MSM Interface allows router management
async fn qmi_command(command: &str) -> common::Result<String> {
    debug!("qmi_command: running command: {:?}", command);

    let process = Command::new(QMI_BINARY)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-d")
        .arg(QMI_DEVICE)
        .arg(command)
        .spawn()?;
    let output = process.wait_with_output().await?;

    debug!("qmi_command: command status: {:?}", output.status.code());
    output.status.success().then(|| ()).ok_or(common::Error::SystemCommandExecutionError)?;
    let output_message = String::from_utf8(output.stdout).map_err(|_| { common::Error::SystemCommandExecutionError })?;
    debug!("qmi_command: command output message:\n {:?}", output_message);
    Ok(output_message)
}
