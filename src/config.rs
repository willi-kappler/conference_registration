use std::net::{SocketAddrV4, Ipv4Addr};
use std::str::FromStr;

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

pub fn load_configuration(file_name: &str) -> Configuration {
    let host = "0.0.0.0";
    let port = 2200;
    let db_filename = "registration_database.sqlite3";
    let template_folder = "templates/";
    let socket_addr = SocketAddrV4::new(Ipv4Addr::from_str(&host).unwrap(), port);
    let email_from = "".to_string();
    let email_server = "".to_string();
    let email_hello = "".to_string();
    let email_username = "".to_string();
    let email_password = "".to_string();

    // TODO: load config from file
    
    Configuration {
        host: host.to_string(),
        port: port,
        socket_addr: socket_addr,
        db_filename: db_filename.to_string(),
        template_folder: template_folder.to_string(),
        email_from: email_from,
        email_server: email_server,
        email_hello: email_hello,
        email_username: email_username,
        email_password: email_password
    }
}
