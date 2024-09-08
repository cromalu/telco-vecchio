use std::net::IpAddr;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Application {
    pub name: String,
    pub host_ip: IpAddr,
    pub port: i32,
    pub end_point: String
}

