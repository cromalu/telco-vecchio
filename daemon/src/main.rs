mod common;
mod sms_utils;
mod email_utils;
mod ssh_utils;
mod user;
mod application;
mod request;
mod init;
mod status;

use std::env;
use std::process::ExitCode;
use std::time::Duration;
use fork::{daemon, Fork};
use log::{debug, error, info};
use crate::common::{Context, Error};
use crate::init::init;
use crate::sms_utils::OutgoingSms;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let arg = args.get(1).map(|s| s.as_str());
    match arg {
        None => {
            run(false).await;
            ExitCode::SUCCESS
        },
        Some("--daemon") => {
            if let Ok(Fork::Child) = daemon(false, false) {
                run(true).await;
            }
            ExitCode::SUCCESS
        },
        Some("--check-config") => {
            if let Some(path) = args.get(2){
                if let Ok(_) = init::read_config_file(path){
                    println!("Valid configuration");
                    ExitCode::SUCCESS
                }else{
                    println!("Invalid configuration");
                    ExitCode::FAILURE
                }
            }else{
                println!("No configuration provided");
                ExitCode::FAILURE
            }
        },
        _ => {
            println!("invalid input arguments");
            ExitCode::FAILURE
        }
    }
}

async fn run(is_daemon: bool) {
    let task = tokio::spawn(async move {
        match init(is_daemon).await {
            Ok(mut context) => {
                loop {
                    debug!("waiting for SMS....");
                    let tunnel_refresh_duration = Duration::from_secs(context.configuration.ssh_config.tunnel_refresh_period_sec);
                    let wait_result = tokio::time::timeout(tunnel_refresh_duration,sms_utils::wait_sms(&context.configuration.sms_config)).await;
                    debug!("SMS waiting interrupted...");
                    match wait_result {
                        Err(_) => {
                            debug!("Tunnel refresh required");
                            context.clean_up_expired_tunnels().await;
                            debug!("Tunnels refreshing done");
                        },
                        Ok(sms_reception_result) => {
                            debug!("New SMS received");
                            match sms_reception_result {
                                Ok(sms) => {
                                    let response = match request::handle_request(sms.from.as_str(), sms.msg.as_str(), &mut context).await {
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
                                        match sms_utils::send_sms(&context.configuration.sms_config,
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
                                Err(e) => {
                                    error!("SMS listening failed {:?}, retrying",e);
                                }
                            }
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




