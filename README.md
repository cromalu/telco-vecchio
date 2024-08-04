# telco-vecchio

## Introduction

telco-vecchio is a package for open-wrt distributions allowing to trigger remote commands on 
a 4G router through SMSs. 
The following commands are supported:
* getting router's current status
* rebooting router
* opening an ssh tunnel between an application running on a host of router's local network and a tunelling service (such as ngrok),
 so that this application can be accessed remotely even if the router is connected to the internet behind a private and non-static ip address

## Building telco-vecchio package

So far project targets only routers with a MIPS 24K CPU and a musl-libc installed

Use Rust stable toolchain `1.70.0`, more recent toolchains are not maintained for MIPS architecture

Install target `mips-unknown-linux-musl`

Download `mips-linux-muslsf-cross` toolchain from http://musl.cc/#binaries 
and configure `<archive root>/bin/mips-linux-muslsf-gcc` as linker for  `mips-unknown-linux-musl` target

build release variant (so that output binaries remains small enough)
`cargo +1.70.0-x86_64-unknown-linux-gnu build --target mips-unknown-linux-musl --release`

## Installing telco-vecchio package


disable any sms-processing binary installed on the router, such as smsd or smstool3

scp the generated daemon to the router on /usr/bin


## telco-vecchio daemon

Upon installation telco-vecchio package deploys a daemon that starts upon system boots.
The daemon monitors incoming SMSs and trigger specific commands based on their content. 

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

This command is triggered by sending to the router an SMS with the following content: `open-tunnel <application-name>`
with <application-name> being the name of the application to connect to, as defined in telco-vecchio daemon configuration file.

The routers sets up a new ssh tunnel with the tunelling service and redirects tunnel's output to the requested application.
The access url generated for this tunnel is sent to the requesting user in an email.

Then, a SMS is sent to the requesting user:
* in case of failure, the SMS details the failure reason
* in case of success, the SMS contains a tunnel-id, identifying the newly created tunnel 

#### Closing a tunnel with an application running on router's local network

This command is triggered by sending to the router an SMS with the following content: `close-tunnel <tunnel-id>`
with <tunnel-id> being the identifier of the tunnel to close.

The routers closes the associated tunnel with the tunelling service, at this point the associated tunnel access url becomes obsolete. 

Then, a SMS is sent to the requesting user:
* in case of failure, the SMS details the failure reason
* in case of success, the SMS confirms that tunnel has been closed

### Configuration

telco-vecchio daemon gets configured at launch time by parsing `...` configuration file having the following parameters.

#### main parameters

* `users`: list of the remote users allowed to interact with telco-vecchio daemon, 
each user is defined with:
** a name
** a phone number
** an email address
Any incoming SMS whose sender phone number does not belong to a user configured in this list is ignored.
Tunnel access urls, generated upon tunnel opening, are sent to the tunnel requesting user through an email.

* `applications`: list of the applications on hosts of router's local network whose remote access is provided by telco-vecchio daemon,
each service is defined with:
** a name
** an ip address, the ip address of the host on router's local network
** a port, the port of the host on which the application is deployed 

#### ssh tunnels parameters

* `tunnel-lifetime`: defines how long a tunnel can remain open, in seconds

* `tunnelling-service-url`: the url to reach out the tunnelling service 

todo

### logfile

todo


 