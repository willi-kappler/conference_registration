use std::net::{SocketAddrV4, Ipv4Addr, AddrParseError};
use std::str::FromStr;
use std::num::ParseIntError;

use ini::Ini;
use ini;

#[derive(Clone)]
pub struct Configuration {
    pub host: String,
    pub port: u16,
    pub socket_addr: SocketAddrV4,
    pub db_filename: String,
    pub template_folder: String,
    pub email_from: String,
    pub email_server: String,
    pub email_hello: String,
    pub email_username: String,
    pub email_password: String
}

pub enum ConfigError {
    Ini,
    Value,
    IP,
}

impl From<ini::ini::Error> for ConfigError {
    fn from(_: ini::ini::Error) -> ConfigError {
        ConfigError::Ini
    }
}

impl From<ParseIntError> for ConfigError {
    fn from(_: ParseIntError) -> ConfigError {
        ConfigError::Value
    }
}

impl From<AddrParseError> for ConfigError {
    fn from(_: AddrParseError) -> ConfigError {
        ConfigError::IP
    }
}

pub fn load_configuration(file_name: &str) -> Result<Configuration, ConfigError> {
    let ini_conf = try!(Ini::load_from_file(file_name));

    let section1 = try!(ini_conf.section(Some("Basic")).ok_or(ConfigError::Ini));
    let host = try!(section1.get("host").ok_or(ConfigError::Ini));
    let port = try!(try!(section1.get("port").ok_or(ConfigError::Ini)).parse::<u16>());
    let db_filename = try!(section1.get("db_filename").ok_or(ConfigError::Ini));
    let template_folder = try!(section1.get("template_folder").ok_or(ConfigError::Ini));
    let host_ip = try!(Ipv4Addr::from_str(&host));
    let socket_addr = SocketAddrV4::new(host_ip, port);

    let section2 = try!(ini_conf.section(Some("EMail")).ok_or(ConfigError::Ini));
    let email_from = try!(section2.get("from").ok_or(ConfigError::Ini));
    let email_server = try!(section2.get("server").ok_or(ConfigError::Ini));
    let email_hello = try!(section2.get("hello").ok_or(ConfigError::Ini));
    let email_username = try!(section2.get("username").ok_or(ConfigError::Ini));
    let email_password = try!(section2.get("password").ok_or(ConfigError::Ini));
    
    Ok(Configuration {
        host: host.to_string(),
        port: port,
        socket_addr: socket_addr,
        db_filename: db_filename.to_string(),
        template_folder: template_folder.to_string(),
        email_from: email_from.to_string(),
        email_server: email_server.to_string(),
        email_hello: email_hello.to_string(),
        email_username: email_username.to_string(),
        email_password: email_password.to_string()
    })
}
