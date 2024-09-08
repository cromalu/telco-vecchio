use std::io::Cursor;
use std::net::IpAddr;
use std::process::Stdio;
use std::time::Duration;
use log::{debug, error};
use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::time::timeout;
use crate::common;
use crate::common::Error;
use crate::common::Error::SshTunnelServiceError;

const SSH_CLOUD_SERVICE_ARGS: [&str; 1] = ["http"];

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SshConfig {
    pub binary_file: String,
    pub key_file: String,
    pub service_user: String,
    pub service_host: String,
    pub tunnel_input_port: i32,
    pub tunnel_setup_timeout_sec: u64,
    pub tunnel_timeout_sec: u64,
    pub tunnel_refresh_period_sec: u64,
}

///SSH tunneling is done through dropbear pre-installed binary on host,
///default keys are configured in /etc/dropbear/
/// returns the process running the ssh tunnel and the tunnel access url on the cloud service
pub async fn setup_ssh_tunnel(config: &SshConfig, output_host: &IpAddr, output_port: i32) -> common::Result<(String, Child)> {
    let mut ssh_command = Command::new(&config.binary_file);
    ssh_command
        .kill_on_drop(true) //allow proper housekeeping in case daemon is killed
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-y")
        .arg("-i")
        .arg(&config.key_file)
        .arg("-R")
        .arg(format!("{}:{}:{}", &config.tunnel_input_port, output_host, output_port))
        .arg(format!("{}@{}", &config.service_user, &config.service_host))
        .arg(SSH_CLOUD_SERVICE_ARGS.join(" "));
    debug!("setup_ssh_tunnel: command: {:?}",ssh_command);
    let mut ssh_process = ssh_command
        .spawn()?;
    debug!("setup_ssh_tunnel: command issued");

    //reading tunnel url from stdout
    let mut stdout = ssh_process.stdout.take().ok_or(common::Error::SystemCommandExecutionError)?;
    let tunnel_url = timeout(
        Duration::from_secs(config.tunnel_setup_timeout_sec),
        async {
            let mut cursor = Cursor::new(vec![0u8; 300]);
            let regexp_success = Regex::new(r#"Forwarding  (.+)\n"#).unwrap();
            let regexp_error = Regex::new(r#"(ERR_[a-zA-Z0-9_]+)"#).unwrap();
            loop {
                let mut chunk_buf = vec![0u8; 30];
                debug!("setup_ssh_tunnel: waiting for service response");
                if let Ok(len) = stdout.read(&mut chunk_buf).await {
                    debug!("setup_ssh_tunnel: service response received, chunk size: {}",len);
                    //appending the content of the chunk read to cursor and try to read url from cursor
                    let _ = cursor.write(&chunk_buf[0..len]).await?;
                    if let Ok(s) = String::from_utf8(cursor.get_ref().clone()) {
                        debug!("setup_ssh_tunnel: concatenated process output: {}",s);
                        if let Some(captures) = regexp_success.captures(&s) {
                            let [url] = captures.extract().1.map(|s| s.to_string());
                            debug!("setup_ssh_tunnel: url read: {}",url);
                            break Ok(url);
                        } else if let Some(captures) = regexp_error.captures(&s) {
                            let [error] = captures.extract().1.map(|s| s.to_string());
                            debug!("setup_ssh_tunnel: error: {}",error);
                            break Err(SshTunnelServiceError(error));
                        } else {
                            debug!("setup_ssh_tunnel: url not yet read");
                        }
                    } else {
                        error!("setup_ssh_tunnel: cannot read process output");
                        break Err(Error::SshTunnelUrlParsingError);
                    }
                } else {
                    error!("setup_ssh_tunnel: cannot read process output");
                    break Err(Error::SystemCommandExecutionError);
                }
            }
        },
    ).await.unwrap_or_else(|_| {
        error!("setup_ssh_tunnel: timeout while trying to read tunnel url");
        Err(Error::SshTunnelUrlSetupTimeout)
    });
    if let Err(_) = tunnel_url {
        let mut stderr = ssh_process.stderr.take().ok_or(common::Error::SystemCommandExecutionError).unwrap();
        let mut buff = vec![0u8; 300];
        let len = stderr.read(&mut buff).await.unwrap();
        error!("setup_ssh_tunnel: stderr: {}",String::from_utf8(buff[0..len].to_vec()).unwrap());
    }
    Ok((tunnel_url?, ssh_process))
}

