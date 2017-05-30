use std::net::{SocketAddrV4, Ipv4Addr, AddrParseError};
use std::str::FromStr;
use std::num::ParseIntError;

use ini::Ini;
use ini;

#[derive(Clone, Debug, PartialEq)]
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
    pub email_password: String,
    pub course1: String,
    pub course2: String
}

#[derive(Debug)]
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
    let ini_conf = Ini::load_from_file(file_name)?;

    let section1 = ini_conf.section(Some("Basic")).ok_or(ConfigError::Ini)?;
    let host = section1.get("host").ok_or(ConfigError::Ini)?;
    let port = section1.get("port").ok_or(ConfigError::Ini)?.parse::<u16>()?;
    let db_filename = section1.get("db_filename").ok_or(ConfigError::Ini)?;
    let template_folder = section1.get("template_folder").ok_or(ConfigError::Ini)?;
    let host_ip = Ipv4Addr::from_str(&host)?;
    let socket_addr = SocketAddrV4::new(host_ip, port);

    let section2 = ini_conf.section(Some("EMail")).ok_or(ConfigError::Ini)?;
    let email_from = section2.get("from").ok_or(ConfigError::Ini)?;
    let email_server = section2.get("server").ok_or(ConfigError::Ini)?;
    let email_hello = section2.get("hello").ok_or(ConfigError::Ini)?;
    let email_username = section2.get("username").ok_or(ConfigError::Ini)?;
    let email_password = section2.get("password").ok_or(ConfigError::Ini)?;
    let course1 = section2.get("course1").ok_or(ConfigError::Ini)?;
    let course2 = section2.get("course2").ok_or(ConfigError::Ini)?;

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
        email_password: email_password.to_string(),
        course1: course1.to_string(),
        course2: course2.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::{load_configuration, Configuration};
    use std::io::BufWriter;
    use std::fs::OpenOptions;
    use std::io::prelude::Write;
    use std::net::{SocketAddrV4, Ipv4Addr};
    use std::str::FromStr;
    use std::fs;

    #[test]
    fn test_load_configuration1() {
        let file_name = "test_config1.ini";

        {
            let mut buffer = BufWriter::new(
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(file_name).unwrap());

            write!(buffer, "
                [Basic]
                host = 127.0.0.1
                port = 1234
                db_filename = my_db.sql
                template_folder = template

                [EMail]
                from = bob@smith.com
                server = some.smtp.com
                hello = my.server.org
                username = bob
                password = secret
                course1 = 1. Jan 2000
                course2 = 12. August 2010
            ").unwrap();
        }

        let config = load_configuration("test_config1.ini").unwrap();

        let expected = Configuration {
            host: "127.0.0.1".to_string(),
            port: 1234,
            socket_addr: SocketAddrV4::new(Ipv4Addr::from_str("127.0.0.1").unwrap(), 1234),
            db_filename: "my_db.sql".to_string(),
            template_folder: "template".to_string(),
            email_from: "bob@smith.com".to_string(),
            email_server: "some.smtp.com".to_string(),
            email_hello: "my.server.org".to_string(),
            email_username: "bob".to_string(),
            email_password: "secret".to_string(),
            course1: "1. Jan 2000".to_string(),
            course2: "12. August 2010".to_string(),
        };

        assert_eq!(config, expected);

        fs::remove_file(file_name).unwrap();
    }
}
