extern crate iron;
extern crate router;
extern crate mount;
extern crate staticfile;
extern crate rusqlite;
extern crate handlebars_iron;
extern crate rustc_serialize;
extern crate params;
extern crate plugin;
#[macro_use] extern crate log;
extern crate simplelog;
extern crate persistent;
extern crate lettre;
extern crate ini;

// System modules

use std::error::Error;
use std::path::Path;
use std::fs::File;

// External modules

use iron::prelude::{Iron, Chain};
use iron::typemap::Key;
use router::Router;
use mount::Mount;
use staticfile::Static;
use rusqlite::Connection;
use handlebars_iron::{HandlebarsEngine, DirectorySource};
use simplelog::{FileLogger, LogLevelFilter};
use persistent::{Read, Write};


// Local modules

mod config;
mod handler;

use config::{load_configuration, Configuration};
use handler::{handle_main, handle_submit};

pub struct DBConnection;

impl Key for DBConnection { type Value = Connection; }

impl Key for Configuration { type Value = Configuration; }

fn main() {
    let config_file = "registration_config.ini";
    let config = match load_configuration(config_file) {
        Ok(configuration) => configuration,
        Err(_) => panic!("Could not open configuration file: '{}'", config_file)
    };

    let _ = FileLogger::init(LogLevelFilter::Info, File::create(&config.log_file).unwrap());

    let db_conn = Connection::open(&config.db_filename).unwrap();

    let mut hbse = HandlebarsEngine::new();
    hbse.add(Box::new(DirectorySource::new(&config.template_folder, ".hbs")));

    if let Err(r) = hbse.reload() {
        panic!("{}", r.description());
    }

    let mut router = Router::new();

    router.get("/", handle_main, "index");
    router.post("/", handle_main, "index");

    router.get("/submit", handle_submit, "submit");
    router.post("/submit", handle_submit, "submit");

    let mut mount = Mount::new();

    mount.mount("/", router);
    mount.mount("/css/", Static::new(Path::new("css/")));

    let mut chain1 = Chain::new(mount);
    chain1.link_after(hbse);

    let mut chain2 = Chain::new(chain1);
    chain2.link(Write::<DBConnection>::both(db_conn));

    let mut chain3 = Chain::new(chain2);
    chain3.link(Read::<Configuration>::both(config.clone()));

    Iron::new(chain3).http(&config.socket_addr).unwrap();
}
