mod common;
mod at_utils;

use std::time::Duration;
use log::info;

fn main() {
    let _ = env_logger::init();

    info!("Waiting for SMS....");
    let res = at_utils::wait_sms(Duration::from_secs(10));
    info!("Result {:?}",res);


}





