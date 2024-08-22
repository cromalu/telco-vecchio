use std::net::IpAddr;
use std::process::Stdio;
use std::time::Duration;
use log::{debug, error};
use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::time::timeout;
use crate::common;
use crate::common::Error;

const SSH_CLOUD_SERVICE_ARGS: [&str; 1] = ["http"];

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SshConfig{
    binary_file : String,
    key_file: String,
    service_user: String,
    service_host: String,
    tunnel_input_port: i32,
    tunnel_setup_timeout_sec: u64
}

///SSH tunneling is done through dropbear pre-installed binary on host,
///default keys are configured in /etc/dropbear/
/// returns the process running the ssh tunnel and the tunnel access url on the cloud service
pub async fn setup_ssh_tunnel(config: &SshConfig, output_host: &IpAddr, output_port: i32) -> common::Result<(String,Child)> {
    let mut ssh_process = Command::new(&config.binary_file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-i")
        .arg(&config.key_file)
        .arg("-R")
        .arg(format!("{}:{}:{}", &config.tunnel_input_port, output_host, output_port))
        .arg(format!("{}@{}", &config.service_user, &config.service_host))
        .arg(SSH_CLOUD_SERVICE_ARGS.join(" "))
        .spawn()?;

    debug!("setup_ssh_tunnel: command issued");

    //reading tunnel url from stdout
    let mut stdout = ssh_process.stdout.take().ok_or(common::Error::SystemCommandExecutionError)?;
    let tunnel_url = timeout(
        Duration::from_secs(config.tunnel_setup_timeout_sec),
        async {
            loop {
                let mut buf = vec![0u8; 100];
                if let Ok(len) = stdout.read(&mut buf).await{
                    if let Ok(s) = String::from_utf8(buf[0..len].to_vec()) {
                        if !s.is_empty(){
                            debug!("setup_ssh_tunnel: process output: {}",s);
                            let re = Regex::new(r#"Forwarding  (.+)\n"#).unwrap();
                            if let Some(captures) = re.captures(&s){
                                let [url] = captures.extract().1.map(|s| s.to_string());
                                debug!("setup_ssh_tunnel: url read: {}",url);
                                break Ok(url)
                            }
                        }
                    }else{
                        error!("setup_ssh_tunnel: cannot read process output");
                        break Err(Error::SshTunnelUrlParsingError)
                    }
                }else{
                    error!("setup_ssh_tunnel: cannot read process output");
                    break Err(Error::SshTunnelUrlParsingError)
                }
            }
        },
    ).await.unwrap_or_else(|_| {
        error!("setup_ssh_tunnel: timeout while trying to read tunnel url");
        Err(Error::SshTunnelUrlParsingError)
    })?;

    Ok((tunnel_url,ssh_process))
}