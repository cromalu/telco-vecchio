use std::net::IpAddr;
use serde::{Deserialize, Serialize};
use crate::common;
use crate::common::Error;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Application {
    pub name: String,
    pub host: IpAddr,
    pub port: i32,
}


pub fn init(raw_string: &str) -> common::Result<Vec<Application>> {
    let deserialized: Vec<Application> = serde_yaml::from_str(&raw_string).map_err(|e| Error::ConfigurationParsingError(e))?;
    Ok(deserialized)
}


