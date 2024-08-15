use std::net::IpAddr;
use std::process::Stdio;
use std::thread::sleep;
use std::time::Duration;
use log::{debug, error};
use regex_lite::Regex;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::time::error::Elapsed;
use tokio::time::timeout;
use crate::common;
use crate::common::Error;

const SSH_BINARY_PATH: &str = "ssh";

const SSH_KEY_FILE: &str = "/etc/dropbear/dropbear_rsa_host_key";
const SSH_CLOUD_SERVICE_USER: &str = "v2";
const SSH_CLOUD_SERVICE_HOST: &str = "connect.ngrok-agent.com";
const SSH_CLOUD_SERVICE_ARGS: [&str; 1] = ["http"];
const SSH_CLOUD_SERVICE_INPUT_PORT: i32 = 0;
const SSH_TUNNEL_SETUP_MAX_DURATION_SECONDS: u64 = 5;

///SSH tunneling is done through dropbear pre-installed binary on host,
///default keys are configured in /etc/dropbear/
/// returns the process running the ssh tunnel and the tunnel access url on the cloud service
pub async fn setup_ssh_tunnel(output_host: &IpAddr, output_port: i32) -> common::Result<(String,Child)> {
    let mut ssh_process = Command::new(SSH_BINARY_PATH)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-i")
        .arg(SSH_KEY_FILE)
        .arg("-R")
        .arg(format!("{}:{}:{}", SSH_CLOUD_SERVICE_INPUT_PORT, output_host, output_port))
        .arg(format!("{}@{}", SSH_CLOUD_SERVICE_USER, SSH_CLOUD_SERVICE_HOST))
        .arg(SSH_CLOUD_SERVICE_ARGS.join(" "))
        .spawn()?;

    debug!("setup_ssh_tunnel: command issued");

    //reading tunnel url from stdout
    let mut stdout = ssh_process.stdout.take().ok_or(common::Error::SystemCommandExecutionError)?;
    let tunnel_url = timeout(
        Duration::from_secs(SSH_TUNNEL_SETUP_MAX_DURATION_SECONDS),
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
    ).await.unwrap_or_else(|e| {
        error!("setup_ssh_tunnel: timeout while trying to read tunnel url");
        Err(Error::SshTunnelUrlParsingError)
    })?;

    Ok((tunnel_url,ssh_process))
}