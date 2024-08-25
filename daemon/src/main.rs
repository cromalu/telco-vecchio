mod common;
mod sms_utils;
mod email_utils;
mod ssh_utils;
mod user;
mod application;
mod request;
mod init;
mod status;

use std::sync::Arc;
use log::{error, info};
use simple_logger::SimpleLogger;
use tokio::sync::Mutex;
use crate::common::{Configuration, Context, Error};
use crate::init::init;
use crate::sms_utils::OutgoingSms;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    SimpleLogger::new().init().unwrap();
    let task = tokio::spawn(async move {
        match init().await {
            Ok(configuration) => {
                info!("initialisation successful, configuration: {:?}",configuration);
                let configuration_ref: &'static Configuration = Box::leak(Box::new(configuration));
                let context = Context::new();
                let shared_context = Arc::new(Mutex::new(context));
                loop {
                    info!("waiting for SMS....");
                    let configuration_ref = configuration_ref.clone();
                    match sms_utils::wait_sms(&configuration_ref.sms_config).await {
                        Ok(sms) => {
                            // A new task is spawned for each incoming sms.
                            let shared_context = Arc::clone(&shared_context);
                            let request_task = tokio::spawn(async move {
                                {
                                    let response = match request::handle_request(sms.from.as_str(), sms.msg.as_str(), &shared_context, configuration_ref).await {
                                        Ok(message) => {
                                            Some(message)
                                        }
                                        Err(Error::SenderNotAllowed(_)) => {
                                            //stay silent
                                            None
                                        }
                                        Err(Error::InvalidRequestError(s)) => {
                                            //applicative error
                                            Some(format!("The message you just sent is invalid : {}", s))
                                        }
                                        Err(e) => {
                                            //technical error
                                            Some(format!("An error occurred : {:?}", e))
                                        }
                                    };

                                    if let Some(message) = response {
                                        info!("Sending back response");
                                        match sms_utils::send_sms(&configuration_ref.sms_config,
                                                                  &OutgoingSms { to: sms.from.to_string(), msg: message }).await {
                                            Ok(()) => {
                                                info!("Response sent");
                                            }
                                            Err(e) => {
                                                error!("Error while sending back response: {:?}",e);
                                            }
                                        }
                                    } else {
                                        info!("No response to send back");
                                    }
                                }
                            });
                            request_task.await.unwrap();
                        }
                        Err(e) => {
                            error!("SMS listening failed {:?}, retrying",e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Initialisation failed {:?}",e);
            }
        };
    });
    task.await.unwrap();
}




