use std::time::Duration;
use log::{debug, error};
use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::sleep;
use crate::common;
use crate::common::Error;
use crate::common::Error::SmsProcessingError;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SmsConfig {
    pub modem_device: String,
}

pub async fn init(config: &SmsConfig) -> common::Result<()> {
    let mut device = File::options().write(true).read(true).open(&config.modem_device).await?;
    debug!("init: setting text mode");
    let _ = device.write("AT+CMGF=1\r".as_bytes()).await?;
    let _ = read_from_file(&mut device, "OK").await?;
    debug!("init: asking for sms forwarding");
    //defines how new messages are indicated
    //first int : defines how notifications are dispatched. Value : 2 -> send notifications to the TE, buffering them and sending them later if they cannot be sent.
    //second int : defines how sms are stored. Value : 2 -> sms not stored on modem, simply forwarded on serial port
    let _ = device.write("AT+CNMI=2,2\r".as_bytes()).await?;
    let _ = read_from_file(&mut device, "OK").await?;
    //tempo in order to be sure that modem is properly configured
    sleep(Duration::from_secs(10)).await;
    Ok(())
}

pub async fn clear(config: &SmsConfig) -> common::Result<()> {
    let mut device = File::options().write(true).read(true).open(&config.modem_device).await?;
    debug!("clear: clearing sms stored");
    let _ = device.write("AT+CMGD=,4\r".as_bytes()).await?;
    let _ = read_from_file(&mut device, "OK").await?;
    Ok(())
}

pub async fn send_sms(config: &SmsConfig, sms: &OutgoingSms) -> common::Result<()> {
    let mut device = File::options().write(true).read(true).open(&config.modem_device).await?;
    debug!("send_sms: setting destination number");
    let _ = device.write(format!("AT+CMGS=\"{}\"\r", sms.to).as_bytes()).await?;
    let _ = read_from_file(&mut device, ">").await?;
    debug!("send_sms: setting sms content");
    let _ = device.write(format!("{}\x1A", sms.msg).as_bytes()).await?;
    let _ = read_from_file(&mut device, "+CMGS").await?;
    debug!("send_sms: sms sent");
    Ok(())
}

pub async fn wait_sms(config: &SmsConfig) -> common::Result<IncomingSms> {
    let mut device = File::options().write(true).read(true).open(&config.modem_device).await?;
    debug!("wait_sms: waiting....");
    let sms_string = read_from_file(&mut device, "+CMT").await?;
    debug!("wait_sms: message received {:?}",sms_string);
    let re = Regex::new(r#"CMT: "(.+)",,"(.+)"\r\n(.*)\r\n"#).unwrap();
    let (_, [from, _date, msg]) = re.captures(&sms_string).ok_or(Error::IncomingSmSParsingError)?.extract();
    let msg = msg.escape_default();
    Ok(IncomingSms { from: from.to_string(), msg: msg.to_string() })
}


#[derive(Debug)]
pub struct IncomingSms {
    pub from: String,
    pub msg: String,
}


#[derive(Debug)]
pub struct OutgoingSms {
    pub to: String,
    pub msg: String,
}

async fn read_from_file(file: &mut File, expected: &str) -> common::Result<String> {
    let mut buffer: [u8; 128] = [0; 128];
    let regexp_error = Regex::new(r#"\+CMS ERROR: (.+)\r\n"#).unwrap();
    loop {
        if let Ok(len) = file.read(&mut buffer).await {
            let s = String::from_utf8(buffer[0..len].to_vec());
            if let Ok(value) = s {
                if !value.is_empty() {
                    debug!("read_from_file : content received: {:?}", value);
                    if value.contains(expected) {
                        return Ok(value);
                    }
                    if let Some(captures) = regexp_error.captures(&value) {
                        let [error] = captures.extract().1.map(|s| s.to_string());
                        error!("read_from_file : error received: {}", error);
                        return Err(SmsProcessingError(error))
                    }
                }
            }
        }
    }
}
