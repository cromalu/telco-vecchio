use std::process::Command;
use std::time::Duration;
use log::{debug, error, info};
use crate::{common, Context, email_utils, init, ssh_utils};
use crate::common::{Error, Tunnel};
use crate::email_utils::OutgoingEmail;
use crate::status::{DeviceStatus, get_status, ServiceStatus};


///Returns the message to be returned to the request sender as acknowledgement
pub async fn handle_request(sender: &str, request: &str, context: &mut Context) -> common::Result<String> {
    info!("handle_request - request received - sender {:?} - request {:?}",sender,request);

    //check if allowed user
    let user = context.configuration.users.iter().filter(|user| { user.phone_number == sender }).next().ok_or_else(|| {
        error!("handle_request - sender is not allowed");
        Error::SenderNotAllowed(sender.to_string())
    })?;

    info!("handle_request - sms received from allowed sender {}",user.name);

    //check request content
    let mut args = request.split_whitespace();
    let command = args.next().ok_or_else(|| {
        error!("handle_request - cannot read command from request");
        Error::InvalidRequestError(format!("Cannot read command from request: {}", request))
    })?;

    match command.to_lowercase().as_str() {
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
                return Err(Error::InvalidStatus(format!("Device is not ready - current state is {}",context.status.device_status)));
            }
            if !matches!(context.status.email_service_status,ServiceStatus::Reachable) {
                error!("handle_request - cannot open tunnel: email service is not reachable");
                return Err(Error::InvalidStatus(format!("Email service is not reachable")));
            }
            if !matches!(context.status.ssh_tunnel_service_status,ServiceStatus::Reachable) {
                error!("handle_request - cannot open tunnel: ssh tunnel service is not reachable");
                return Err(Error::InvalidStatus(format!("SSH tunnel service is not reachable")));
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
                return Err(Error::InvalidStatus(format!("Application {} is not reachable",context.status.device_status)));
            }
            info!("handle_request - opening tunnel to application : {}",application.name);

            if let Some((tunnel_ref,_)) = context.tunnels.iter().find(|(_, tunnel)| {
                tunnel.user == user.name && tunnel.application == application.name
            }){
                error!("handle_request - a tunnel is already open by the user for this application");
                return Err(Error::InvalidRequestError(format!("A tunnel is already open for this application: {}",*tunnel_ref)));
            }

            //open ssh tunnel towards this app
            let (mut tunnel_url, tunnel_process) = ssh_utils::setup_ssh_tunnel(&context.configuration.ssh_config, &application.host_ip, application.port).await?;
            info!("handle_request - tunnel open, url: {}", tunnel_url);

            //appending end point to the generated url
             tunnel_url.push_str(&application.end_point);

            //sending tunnel url to user through email
            email_utils::send_email(&context.configuration.email_config, &OutgoingEmail {
                to: user.email.clone(),
                title: "Tunnel URL".to_string(),
                msg: format!("Hello {} !\nHere is the url to access to {}:\n\n{}\n\nHave a nice day!", user.name, application.name, tunnel_url),
            }).await?;
            info!("handle_request - tunnel url sent by mail to: {}",user.email);

            let process_id = context.tunnels.len() as u32;
            context.tunnels.insert(process_id, Tunnel::new(user.name.clone(), application.name.clone(), tunnel_process));

            //todo indicate the mail in the ack, but masking it
            Ok(format!("Tunnel has been setup, reference is: {}\nAccess url has been send to you by mail", process_id))
        }

        "close" => {
            info!("handle_request - closing tunnel");

            //reading tunnel process reference
            let references = if let Some(s) = args.next() {
                //parsing tunnel process reference to int
                let reference: u32 = s.parse::<u32>().map_err(|_| {
                    error!("handle_request - invalid tunnel reference");
                    Error::InvalidRequestError(format!("Invalid tunnel reference: {}", s))
                })?;
                vec!(reference)
            } else {
                debug!("handle_request - no tunnel reference specified, checking tunnels open by user");
                //if no reference passed, closing all the tunnels open by the user
                let refs: Vec<u32> = context.tunnels.iter().filter_map(|(key, value)| {
                    if value.user == user.name {
                        Some(*key)
                    } else {
                        None
                    }
                }).collect();
                if refs.is_empty() {
                    error!("handle_request - no tunnel reference found");
                    Err(Error::InvalidRequestError(format!("No open tunnel")))
                } else {
                    Ok(refs)
                }?
            };

            info!("handle_request - closing tunnel(s) with reference(s): {:?}",references);

            //resolve process
            for reference in &references {
                let mut entry = context.tunnels.remove(&reference).ok_or_else(|| {
                    error!("handle_request - unknown application");
                    Error::InvalidRequestError(format!("Unknown tunnel reference: {}", reference))
                })?;
                //killing it
                entry.process.kill().await?;
                info!("handle_request - tunnel process with reference: {} has been killed",reference);
            }

            let message = if references.len() > 1 {
                "Tunnels have been closed"
            } else {
                "Tunnel has been closed"
            };
            Ok(message.to_string())
        }

        "status" => {
            info!("handle_request - resolve status");
            let status = get_status(&context.configuration).await?;

            let status_printed = status.to_string();
            //updating available applications with the latest status
            context.update_status(status);

            Ok(status_printed)
        }

        "reboot" => {
            info!("handle_request - reboot");

            init::register_init_listener(&user);
            let _ = tokio::spawn(
                async move {
                    //delay before rebooting so that answer can be returned to sender
                    info!("handle_request - rebooting in 5 secs");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    _ = Command::new("reboot")
                        .spawn()
                }
            );
            Ok("Rebooting...".to_string())
        }

        "shutdown" => {
            info!("handle_request - shutdown");

            let _ = tokio::spawn(
                async move {
                    //delay before rebooting so that answer can be returned to sender
                    info!("handle_request - shutingdown in 5 secs");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    _ = Command::new("poweroff")
                        .spawn()
                }
            );
            Ok("Shutting down...".to_string())
        }

        _ => {
            error!("handle_request - unknown command: {:?}", command);
            Err(Error::InvalidRequestError(format!("Unknown command: {}", command)))
        }
    }
}
