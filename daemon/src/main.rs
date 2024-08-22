mod common;
mod sms_utils;
mod email_utils;
mod ssh_utils;
mod user;
mod application;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use log::{error, info};
use serde::{Deserialize, Serialize};
use simple_logger::SimpleLogger;
use tokio::process::Child;
use tokio::sync::Mutex;
use crate::application::Application;
use crate::common::Error;
use crate::common::Error::ConfigurationParsingError;
use crate::email_utils::{EmailConfig, OutgoingEmail};
use crate::sms_utils::{OutgoingSms, SmsConfig};
use crate::ssh_utils::SshConfig;
use crate::user::User;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    SimpleLogger::new().init().unwrap();
    let task = tokio::spawn(async move {
        match init() {
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
                                    let response = match handle_request(sms.from.as_str(), sms.msg.as_str(), &shared_context, configuration_ref).await {
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

#[derive(Debug)]
struct Context {
    running_processes: HashMap<u32, Child>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Configuration {
    #[serde(rename = "user")]
    pub users: Vec<User>,
    #[serde(rename = "application")]
    pub applications: Vec<Application>,

    pub sms_config: SmsConfig,
    pub email_config: EmailConfig,
    pub ssh_config: SshConfig
}

impl Context {
    fn new() -> Self {
        Self {
            running_processes: HashMap::new(),
        }
    }

    fn store_process(&mut self, process: Child) -> u32 {
        let idx = self.running_processes.len() as u32;
        self.running_processes.insert(idx, process);
        idx
    }

    fn get_process(&mut self, idx: u32) -> Option<Child> {
        self.running_processes.remove(&idx)
    }
}

fn init() -> common::Result<Configuration> {
    info!("initialising...");

    let mut configuration_string = String::new();
    File::open("/etc/telco-vecchio.conf")?.read_to_string(&mut configuration_string)?;
    let configuration: Configuration = toml::from_str(&configuration_string).map_err(|e| ConfigurationParsingError(e))?;

    Ok(configuration)
}

///Returns the message to be returned to the request sender as acknowledgement
async fn handle_request(sender: &str, request: &str, context: &Arc<Mutex<Context>>, configuration: &Configuration) -> common::Result<String> {
    info!("handle_request - request received - sender {:?} - request {:?}",sender,request);

    let mut context = context.lock().await;

    //check if allowed user
    let user = configuration.users.iter().filter(|user| { user.phone_number == sender }).next().ok_or_else(|| {
        error!("handle_request - sender is not allowed");
        Error::SenderNotAllowed(sender.to_string())
    })?;

    info!("handle_request - sms received from allowed sender {}",user.name);

    //check request content
    let mut args = request.split(" ");
    let command = args.next().ok_or_else(|| {
        error!("handle_request - cannot read command from request");
        Error::InvalidRequestError(format!("Cannot read command from request: {}", request))
    })?;

    match command {
        "open" => {
            info!("handle_request - opening tunnel");

            //reading application to open the tunnel
            let application_str = args.next().ok_or_else(|| {
                error!("handle_request - no application specified");
                Error::InvalidRequestError(format!("No application specified: {}", request))
            })?;

            //resolve application
            let application = configuration.applications.iter().filter(|app| { app.name == application_str }).next().ok_or_else(|| {
                error!("handle_request - unknown application");
                Error::InvalidRequestError(format!("Unknown application: {}", application_str))
            })?;

            info!("handle_request - opening tunnel to application : {}",application.name);

            //todo close tunnel if already existing for this user&application

            //open ssh tunnel towards this app
            let (tunnel_url, tunnel_process) = ssh_utils::setup_ssh_tunnel(&configuration.ssh_config, &application.host_ip, application.port).await?;
            info!("handle_request - tunnel open, url: {}", tunnel_url);

            //sending tunnel url to user through email
            email_utils::send_email(&configuration.email_config, &OutgoingEmail {
                to: user.email.clone(),
                title: "Tunnel URL".to_string(),
                msg: format!("Hello {} !\nHere is the url to access to {}:\n\n{}\n\nHave a nice day!", user.name, application.name, tunnel_url),
            }).await?;
            info!("handle_request - tunnel url sent by mail to: {}",user.email);

            let process_id = context.store_process(tunnel_process);

            //todo indicate the mail in the ack, but masking it
            Ok(format!("Tunnel has been setup, reference is: {}\nAccess url has been send to you by mail", process_id))
        }

        "close" => {
            info!("handle_request - closing tunnel");

            //reading tunnel process reference
            let reference_str = args.next().ok_or_else(|| {
                error!("handle_request - no tunnel reference specified");
                Error::InvalidRequestError(format!("No tunnel reference specified: {}", request))
            })?;

            //parsing tunnel process reference to int
            let reference: u32 = reference_str.parse::<u32>().map_err(|_| {
                error!("handle_request - invalid tunnel reference");
                Error::InvalidRequestError(format!("Invalid tunnel reference: {}", reference_str))
            })?;

            info!("handle_request - closing tunnel with reference {}",reference);

            //resolve process
            let mut process = context.get_process(reference).ok_or_else(|| {
                error!("handle_request - unknown application");
                Error::InvalidRequestError(format!("Unknown tunnel reference: {}", reference))
            })?;

            //killing it
            process.kill().await?;
            info!("handle_request - tunnel process has been killed");

            Ok("Tunnel has been closed".to_string())
        }

        _ => {
            error!("handle_request - unknown command");
            Err(Error::InvalidRequestError(format!("Unknown command: {}", command)))
        }
    }
}


