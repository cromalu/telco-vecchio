use std::io;
use gsm7::{Gsm7Reader, Gsm7Writer};
use log::{debug, error};
use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::common;
use crate::common::Error;
use crate::common::Error::{SmsInitError, SmsSendingError};

const SMS_VALIDITY_PERIOD: u8 = 1; //10 minutes

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SmsConfig {
    pub modem_device: String,
}

pub async fn init(config: &SmsConfig) -> common::Result<()> {
    let mut device_file = get_device_file(config).await.map_err(|_| SmsInitError)?;
    debug!("init: running AT+CMGF");
    //set mode to PDU mode
    let response = at_transaction(&mut device_file,"AT+CMGF=0\r").await.map_err(|_| SmsInitError)?;
    if !response.contains("OK"){
        error!("AT+CMGF failed - response: {}",response);
        return Err(SmsInitError)
    }
    debug!("init: running AT+CNMI");
    //defines how new messages are indicated
    //first int : defines how notifications are dispatched. Value : 2 -> send notifications to the TE, buffering them and sending them later if they cannot be sent.
    //second int : defines how sms are stored. Value : 2 -> sms not stored on modem, simply forwarded on serial port
    let response = at_transaction(&mut device_file,"AT+CNMI=2,2\r").await.map_err(|_| SmsInitError)?;
    if !response.contains("OK"){
        error!("AT+CNMI failed - response: {}",response);
        return Err(SmsInitError)
    }
    debug!("init: success");
    Ok(())
}

pub async fn send_sms(config: &SmsConfig, sms: &OutgoingSms) -> common::Result<()> {
    if sms.msg.len() > 140 {
        error!("send_sms: sms message too long");
        return Err(SmsSendingError)
    }
    let mut device_file = get_device_file(config).await.map_err(|_| SmsSendingError)?;

    debug!("send_sms: building pdu");
    let encoded_number = encode_phone_number(&sms.to);
    let encoded_message = encode_message(&sms.msg).map_err(|_| SmsSendingError)?;
    let pdu = format!("0011000B91{}0000{:02X?}{:02X?}{}\x1A", encoded_number, SMS_VALIDITY_PERIOD,(((encoded_message.len()/ 2)*8)/7) as u8, encoded_message);
    debug!("send_sms : pdu built: {}",pdu);

    debug!("send_sms: running AT+CMGS");
    let response = at_transaction(&mut device_file,format!("AT+CMGS={}\r", (pdu.len() - 2) / 2).as_str()).await.map_err(|_| SmsSendingError)?;
    if !response.contains(">"){
        error!("AT+CMGS initiation failed - response: {}",response);
        return Err(SmsSendingError)
    }
    error!("AT+CMGS initiation success - sending command");
    let response = at_transaction(&mut device_file,pdu.as_str()).await.map_err(|_| SmsSendingError)?;
    if !response.contains("OK"){
        error!("AT+CMGS command failed - response: {}",response);
        return Err(SmsSendingError)
    }
    debug!("send_sms: sms sent");
    Ok(())
}

pub async fn wait_sms(config: &SmsConfig) -> common::Result<IncomingSms> {
    let mut device_file = get_device_file(config).await.map_err(|_| Error::SmsReadingError)?;
    let re = Regex::new(r#"\+CMT:.*\r\n(.+)\r\n"#).unwrap();
    debug!("wait_sms: waiting CMT response");
    let pdu = loop{
        let response = at_transaction(&mut device_file,"").await.map_err(|_| Error::SmsReadingError)?;
        if !response.is_empty(){
            debug!("wait_sms: content received: {}",response);
            if let Some(capture) = re.captures(&response){
                debug!("wait_sms: CMT response received");
                let [s] =  capture.extract().1;
                break s.to_string()
            }else{
                debug!("wait_sms: ignoring content");
            }
        }
    };
    debug!("wait_sms: parsing pdu");
    let number_encoded = &pdu[22..34];
    let message_encoded = &pdu[54..];
    let number = decode_phone_number(number_encoded);
    let message = decode_message(message_encoded).map_err(|_| Error::SmsReadingError)?;
    debug!("wait_sms: sender number {:?}",number);
    debug!("wait_sms: message content {:?}",message);
    Ok(IncomingSms { from: number.to_string(), msg: message.to_string() })
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

async fn get_device_file(config: &SmsConfig) -> Result<File,io::Error> {
    File::options().write(true).read(true).open(&config.modem_device).await
}

async fn at_transaction(device_file: &mut File, command: &str) -> Result<String,io::Error> {
    if !command.is_empty(){
        debug!("at_transaction: sending command: {:?}",command);
        device_file.write_all(command.as_bytes()).await?;
        //reading the command just sent
        let mut buffer= vec![0;command.as_bytes().len()];
        let _ = device_file.read(&mut buffer).await?;
        debug!("at_transaction: waiting response");
    }
    let mut buffer: [u8; 128] = [0; 128];
    let len = device_file.read(&mut buffer).await?;
    String::from_utf8(buffer[0..len].to_vec()).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))
}

fn encode_phone_number(phone_number: &str) -> String {
    debug!("encode_phone_number: in: {}",phone_number);
    //stripping + and right padding with F
    let paded_number: Vec<char> = format!("{:F<12}", phone_number.replace("+", "")).chars().collect();
    //swap numbers 2 by to
    let mut encoded_numer = vec!();
    paded_number.chunks(2).for_each(|chunk| {
        encoded_numer.push(chunk[1]);
        encoded_numer.push(chunk[0]);
    });
    let number: String = encoded_numer.iter().collect();
    debug!("encode_phone_number: out: {}",number);
    number
}

fn decode_phone_number(encoded_phone_number: &str) -> String {
    debug!("decode_phone_number: in: {}",encoded_phone_number);
    let numbers: Vec<char> = encoded_phone_number.chars().collect();
    //swap numbers 2 by to
    let mut swapped_numbers = vec!();
    numbers.chunks(2).for_each(|chunk| {
        swapped_numbers.push(chunk[1]);
        swapped_numbers.push(chunk[0]);
    });
    let padded_number: String = swapped_numbers.iter().collect();
    let number = format!("+{}", padded_number.replace("F", ""));
    debug!("decode_phone_number: out: {}",number);
    number
}

fn encode_message(message: &str) -> Result<String,io::Error> {
    debug!("encode_message: in: {}",message);
    let mut writer = Gsm7Writer::new(Vec::new());
    writer.write_str(message)?;
    let out = hex::encode(writer.into_writer()?).to_uppercase();
    debug!("encode_message: out: {}",out);
    return Ok(out);
}

fn decode_message(encoded_message: &str) -> Result<String,io::Error> {
    debug!("decode_message: in: {}",encoded_message);
    let v = hex::decode(encoded_message.to_lowercase()).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
    let reader = Gsm7Reader::new(io::Cursor::new(&v));
    let out = reader.collect::<io::Result<String>>()?;
    debug!("decode_message: out: {}",out);
    return Ok(out);
}

