mod common;
mod sms_utils;

use log::info;
use crate::sms_utils::OutgoingSms;

#[tokio::main]
async fn main() {
    let _ = env_logger::init();
    let task = tokio::spawn(async move {
        info!("Waiting for SMS....");
        let res = sms_utils::wait_sms().await;
        info!("Result {:?}",res);
        info!("Sending SMS....");
        //let res = sms_utils::send_sms(&OutgoingSms{ to: "+XXXXXXX".to_string(), msg: "hey you".to_string() }).await;
        info!("Result {:?}",res);
    });
    task.await.unwrap();
}





