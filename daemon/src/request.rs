use std::sync::Arc;
use log::{error, info};
use tokio::sync::Mutex;
use crate::{common, Context, email_utils, ssh_utils};
use crate::common::Error;
use crate::email_utils::OutgoingEmail;
use crate::status::{DeviceStatus, get_status, ServiceStatus};
use crate::status::InvalidStatusKind::{ApplicationNotAvailable, EmailServiceUnreachable, InvalidDeviceStatus, SshTunnelServiceUnreachable};



///Returns the message to be returned to the request sender as acknowledgement
pub async fn handle_request(sender: &str, request: &str, context: &Arc<Mutex<Context>>) -> common::Result<String> {
    info!("handle_request - request received - sender {:?} - request {:?}",sender,request);

    let mut context = context.lock().await;

    //check if allowed user
    let user = context.configuration.users.iter().filter(|user| { user.phone_number == sender }).next().ok_or_else(|| {
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

            info!("handle_request - requested application: {}",application_str);

            //checking if the current status allows tunnel opening
            if !matches!(context.status.device_status,DeviceStatus::Ready) {
                error!("handle_request - cannot open tunnel: device status: {:?}",context.status.device_status);
                return Err(Error::InvalidStatus(InvalidDeviceStatus(context.status.device_status.clone())));
            }
            if !matches!(context.status.email_service_status,ServiceStatus::Reachable) {
                error!("handle_request - cannot open tunnel: email service is not reachable");
                return Err(Error::InvalidStatus(EmailServiceUnreachable));
            }
            if !matches!(context.status.ssh_tunnel_service_status,ServiceStatus::Reachable) {
                error!("handle_request - cannot open tunnel: ssh tunnel service is not reachable");
                return Err(Error::InvalidStatus(SshTunnelServiceUnreachable));
            }

            //resolve application
            let application = context.configuration.applications.iter().filter(|app| { app.name == application_str }).next()
                .ok_or(Error::InvalidRequestError(format!("Unknown application: {}", application_str)))
                .or_else(|e| {
                    error!("handle_request - cannot open tunnel: application {} is unknown",application_str);
                    Err(e)
                })?;
            if !matches!(context.status.applications_status.get(application_str).unwrap_or(&ServiceStatus::Unreachable),ServiceStatus::Reachable) {
                error!("init - cannot open tunnel: application {} is not reachable",application_str);
                return Err(Error::InvalidStatus(ApplicationNotAvailable(application_str.to_string())));
            }
            info!("handle_request - opening tunnel to application : {}",application.name);

            //todo close tunnel if already existing for this user&application

            //open ssh tunnel towards this app
            let (tunnel_url, tunnel_process) = ssh_utils::setup_ssh_tunnel(&context.configuration.ssh_config, &application.host_ip, application.port).await?;
            info!("handle_request - tunnel open, url: {}", tunnel_url);

            //sending tunnel url to user through email
            email_utils::send_email(&context.configuration.email_config, &OutgoingEmail {
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

        "status" => {
            info!("handle_request - resolve status");
            let status = get_status(&context.configuration).await?;

            let status_printed = status.to_string();
            //updating available applications with the latest status
            context.update_status(status);

            Ok(status_printed)
        }

        _ => {
            error!("handle_request - unknown command: {:?}", command);
            Err(Error::InvalidRequestError(format!("Unknown command: {}", command)))
        }
    }
}
