# telco-vecchio

## Introduction

telco-vecchio is a package for open-wrt distributions allowing to trigger remote commands on 
a GL-iNet GL-X300BC4 Collie 4G router through SMSs. 

The following commands are supported:
* getting router's current status
* rebooting router
* opening an ssh tunnel between an application running on a host of router's local network and a tunelling service (such as ngrok),
 so that this application can be accessed remotely even if the router is connected to the internet behind a private and non-static ip address

## Building telco-vecchio package

GL-X300BC4 Collie 4G router is set with a MIPS 24K CPU and a has a musl-libc installed

Use Rust stable toolchain `1.70.0`, more recent toolchains are not maintained for MIPS architecture

Install target `mips-unknown-linux-musl`

Download `mips-linux-muslsf-cross` toolchain from http://musl.cc/#binaries 
and configure `<archive root>/bin/mips-linux-muslsf-gcc` as linker for  `mips-unknown-linux-musl` target

build release variant (so that output binaries remains small enough)
`cargo +1.70.0-x86_64-unknown-linux-gnu build --target mips-unknown-linux-musl --release`

## Administration

### Router Setup

Install the latest firmware build for the router from GL-iNet website.

Disable services that might disturb telco-vecchio access to router's modem:
* smsd
* smstool3
* carrier-monitor

Disable cron jobs invoking `modem.sh`, configured in `/etc/gl_crontabs/` that disturb SMS reception


### Smtp client configuration
Telco-vecchio daemon sends emails relying on a smtp client, pre-installed on host, called ssmtp.
This binary is configured from the following configuration files:

* /etc/ssmtp/ssmpt.conf:
```
#
# /etc/ssmtp.conf -- a config file for sSMTP sendmail.
#

# The person who gets all mail for userids < 1000
# Make this empty to disable rewriting.
root=<sender email>

# The place where the mail goes. The actual machine name is required
# no MX records are consulted. Commonly mailhosts are named mail.domain.com
# The example will fit if you are in domain.com and your mailhub is so named.
mailhub=<email provider smtp server:port>

# Example for SMTP port number 2525
# mailhub=mail.your.domain:2525
# Example for SMTP port number 25 (Standard/RFC)
# mailhub=mail.your.domain
# Example for SSL encrypted connection
# mailhub=mail.your.domain:465

# Where will the mail seem to come from?
rewriteDomain=

# The full hostname
hostname=<sender email>

#Login
AuthUser=<sender login>
#Password
AuthPass=<sender password>


# Set this to never rewrite the "From:" line (unless not given) and to
# use that address in the "from line" of the envelope.
FromLineOverride=YES

# Use SSL/TLS to send secure messages to server.
#UseTLS=NO
UseSTARTTLS=YES

# Use SSL/TLS certificate to authenticate against smtp host.
#UseTLSCert=YES

# Use this RSA certificate.
#TLSCert=/etc/ssl/certs/ssmtp.pem

# Get enhanced (*really* enhanced) debugging information in the logs
# If you want to have debugging of the config file parsing, move this option
# to the top of the config file and uncomment
#Debug=YES
```
* /etc/ssmtp/revaliases
```
root:<sender email>:<email provider smtp server:port>
```

### Ngrok SSH key pair configuration

Setup an account on ngrok website, once created upload router public key on your account. 
A default key pair is defined in the file `/etc/dropbear/dropbear_rsa_host_key`, this file contains both
public and private key, ensure not to upload the private key.

reverse ssh tunnel can be manually set up with the following command
```ssh -i /etc/dropbear/dropbear_rsa_host_key -R 0:127.0.0.1:8080 v2@connect.ngrok-agent.com http```

## Telco-vecchio package installation

* Scp the generated daemon to the router on /usr/bin/telco-vecchio.

* Edit and scp the telco vecchio configuration file on /etc/telco-vecchio.conf, configuration format is described later in this doc.

* In order for the daemon to start at launch time create an init.d script:
```
#!/bin/sh /etc/rc.common
START=95

PROG=/usr/bin/telco-vecchio

start() {
	$PROG
}

stop() {
	return 0
}
```
push it to /etc/init.d/telco-vecchio and invoke `/etc/init.d/telco-vecchio enable` 

## telco-vecchio daemon

Upon installation telco-vecchio package deploys a daemon that starts upon system boots.
The daemon monitors incoming SMSs and trigger specific commands based on their content. 

Logs printed by the daemon are written into /var/log/telco-vecchio* files.

### Commands

Here is the list of the commands that can be run by telco-vecchio daemon:

#### Getting router's current status

This command is triggered by sending to the router an SMS with the following content: `status`

The router replies to the sender with an SMS containing a description of its current state, including:
- the status of the mobile network : signal strength, remaining internet-data volume allowed
- the status of the configured applications on the local network
- the status of the tunnelling-service
- the available space on ram
- the available space on disk

#### Rebooting router

This command is triggered by sending to the router an SMS with the following content: `reboot`

The router first replies to the sender with an SMS indicating that a reboot is going to start, then reboots, 
then sends to the sender a new SMS indicating that a reboot is done.

#### Openning a tunnel with an application running on router's local network

This command is triggered by sending to the router an SMS with the following content: `open <application-name>`
with <application-name> being the name of the application to connect to, as defined in telco-vecchio daemon configuration file.

The routers sets up a new ssh tunnel with the tunelling service and redirects tunnel's output to the requested application.
The access url generated for this tunnel is sent to the requesting user in an email.

Then, a SMS is sent to the requesting user:

. in case of failure, the SMS details the failure reason

. in case of success, the SMS contains a tunnel-id, identifying the newly created tunnel 

#### Closing a tunnel with an application running on router's local network

This command is triggered by sending to the router an SMS with the following content: `close <tunnel-id>`
with <tunnel-id> being the identifier of the tunnel to close.

The routers closes the associated tunnel with the tunelling service, at this point the associated tunnel access url becomes obsolete. 

Then, a SMS is sent to the requesting user:

. in case of failure, the SMS details the failure reason
. in case of success, the SMS confirms that tunnel has been closed

### Configuration

telco-vecchio daemon gets configured at launch time by parsing `...` configuration file having the following parameters.

#### functional parameters

* `users`: list of the remote users allowed to interact with telco-vecchio daemon, 
each user is defined with:
    * a name
    * a phone number
    * an email address
Any incoming SMS whose sender phone number does not belong to a user configured in this list is ignored.
Tunnel access urls, generated upon tunnel opening, are sent to the tunnel requesting user through an email.

A new user is added by adding the following block to the configuration file:

```
[[user]]
name = "..."
phone_number = "+..."
email = "..."
```

* `applications`: list of the applications on hosts of router's local network whose remote access is provided by telco-vecchio daemon,
each service is defined with:
    * a name
    * an ip address, the ip address of the host on router's local network
    * a port, the port of the host on which the application is deployed 

A new application is added by adding the following block to the configuration file:

```
[[application]]
name = "..."
host_ip = "..."
port = ...
```

#### technical parameters

##### sms parameters

todo

##### email parameters

todo

##### ssh tunnels parameters

todo

### logfile

todo


 
