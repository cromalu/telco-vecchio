use std::io;
use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};
use log::debug;
use serialport::SerialPort;
use crate::common;
use crate::common::Error::IoError;

const MODEM_DEVICE : &str = "/dev/ttyUSB2";
const MODEM_BAUD_RATE : u32 = 115200;
const SERIAL_PORT_READ_TIMEOUT : Duration = Duration::from_secs(10);
const AT_COMMAND_TIMEOUT : Duration = Duration::from_secs(10);


pub fn send_sms(to: &str, msg: &str) -> common::Result<()> {
    let mut port = serialport::new(MODEM_DEVICE, MODEM_BAUD_RATE).timeout(SERIAL_PORT_READ_TIMEOUT).open()?;
    debug!("send_sms: setting text mode");
    let _ = port.write("AT+CMGF=1\r".as_bytes())?;
    let _ = listen_on_port(&mut port, "OK", AT_COMMAND_TIMEOUT)?;
    debug!("send_sms: setting destination number");
    let _ = port.write(format!("AT+CMGS=\"{}\"\r", to).as_bytes())?;
    let _ = listen_on_port(&mut port, ">", AT_COMMAND_TIMEOUT)?;
    debug!("send_sms: setting sms content");
    let _ = port.write(format!("{}\x1A", msg).as_bytes())?;
    let _ = listen_on_port(&mut port, "+CMGS", AT_COMMAND_TIMEOUT)?;
    Ok(())
}

fn listen_on_port(port: &mut Box<dyn SerialPort>, expected: &str, timeout: Duration) -> common::Result<String> {
    let start_time = Instant::now();
    let mut buffer: [u8; 128] = [0; 128];
    loop {
        if let Ok(len) = port.read(&mut buffer) {
            let s = String::from_utf8(buffer[0..len].to_vec());
            if let Ok(value) = s {
                debug!("listen_on_port : content received: {:?}", value);
                if value.contains(expected) {
                    return Ok(value.to_string());
                }
            }
        }
        if start_time.elapsed() > timeout {
            return Err(IoError(io::Error::from(ErrorKind::TimedOut)))
        }
    }
}
