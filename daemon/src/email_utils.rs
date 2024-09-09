use std::net::IpAddr;
use std::process::Stdio;
use log::debug;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use crate::common;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct EmailConfig {
    pub binary_file: String,
    pub sender_alias: String,
    pub server_domain: String,
    pub internet_host: IpAddr,
}

#[derive(Debug)]
pub struct OutgoingEmail {
    pub to: String,
    pub title: String,
    pub msg: String,
}

///Email sending is done through sendmail pre-installed binary on host,
/// configured from /etc/ssmtp/ssmpt.conf and /etc/ssmtp/revaliases configuration files
pub async fn send_email(config: &EmailConfig, email: &OutgoingEmail) -> common::Result<()> {
    let content = format!("Subject: {}\n\n{}", email.title, email.msg);


    let mut send_mail_process = Command::new(&config.binary_file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-v")
        .arg(&email.to)
        .arg("-F")
        .arg(&config.sender_alias)
        .spawn()?;
    let mut stdin = send_mail_process.stdin.take().ok_or(common::Error::SystemCommandExecutionError)?;
    let mut stderr = send_mail_process.stderr.take().ok_or(common::Error::SystemCommandExecutionError)?;
    stdin
        .write(content.as_bytes())
        .await?;
    // We drop the handle here which signals EOF to the child process.
    // This tells the child process that it there is no more data on the pipe.
    drop(stdin);

    let output = send_mail_process.wait_with_output().await?;

    //send mail output is printed to stderr
    let mut s = String::new();
    let _ = stderr.read_to_string(&mut s).await?;
    debug!("send_email: command output :\n{}", s);

    output.status.success().then(|| ()).ok_or(common::Error::SystemCommandExecutionError)
}