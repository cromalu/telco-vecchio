use serde::{Deserialize, Serialize};
use crate::common;
use crate::common::Error;


#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub phone_number: String,
    pub email: String,
}


pub fn init(raw_string: &str) -> common::Result<Vec<User>> {
    let deserialized: Vec<User> = serde_yaml::from_str(&raw_string).map_err(|e| Error::ConfigurationParsingError(e))?;
    Ok(deserialized)
}


