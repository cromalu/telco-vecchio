use std::sync::Arc;
use log::{error, info};
use tokio::sync::Mutex;
use crate::{common, Configuration, Context, email_utils, ssh_utils};
use crate::common::Error;
use crate::email_utils::OutgoingEmail;

///Returns the message to be returned to the request sender as acknowledgement
pub async fn handle_request(sender: &str, request: &str, context: &Arc<Mutex<Context>>, configuration: &Configuration) -> common::Result<String> {
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
