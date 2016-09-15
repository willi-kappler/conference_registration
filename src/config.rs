use std::net::{SocketAddrV4, Ipv4Addr};
use std::str::FromStr;


pub struct Configuration {
    pub host: String,
    pub port: u16,
    pub socket_addr: SocketAddrV4,
    pub db_filename: String,
    pub template_folder: String
}

pub fn load_configuration(file_name: &str) -> Configuration {
    let host = "0.0.0.0";
    let port = 2200;
    let db_filename = "registration_database.sqlite3";
    let template_folder = "templates/";
    let socket_addr = SocketAddrV4::new(Ipv4Addr::from_str(&host).unwrap(), port);

    Configuration {
        host: host.to_string(),
        port: port,
        socket_addr: socket_addr,
        db_filename: db_filename.to_string(),
        template_folder: template_folder.to_string()
    }
}
