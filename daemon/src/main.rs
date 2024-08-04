mod common;
mod at_utils;

use log::info;

fn main() {
    let _ = env_logger::init();
    info!("Sending SMS....");
    //change with correct number
    let res = at_utils::send_sms("+XXXXXX", "what's up?");
    info!("Result {:?}",res);
}





