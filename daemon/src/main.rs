mod common;
mod at_utils;

use log::info;
use crate::at_utils::OutgoingSms;

#[tokio::main]
async fn main() {
    let _ = env_logger::init();
    let task = tokio::spawn(async move {
        info!("Waiting for SMS....");
        let res = at_utils::wait_sms().await;
        info!("Result {:?}",res);
        info!("Sending SMS....");
        //let res = at_utils::send_sms(&OutgoingSms{ to: "+XXXXXXX".to_string(), msg: "hey you".to_string() }).await;

        info!("Result {:?}",res);
    });
    task.await.unwrap();
}





