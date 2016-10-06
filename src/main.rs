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

// System modules

use std::error::Error;
use std::path::Path;
use std::fs::File;

// External modules

use iron::prelude::{Iron, Chain};

use router::Router;
use mount::Mount;
use staticfile::Static;
use rusqlite::{SqliteConnection};
use handlebars_iron::{HandlebarsEngine, DirectorySource};
use simplelog::{FileLogger, LogLevelFilter};

// Local modules

mod config;
mod handler;



use config::load_configuration;
use handler::{handle_main, handle_submit};

fn main() {

    // TODO: look at persistent:
    // https://github.com/iron/persistent/blob/master/examples/hitcounter.rs

    let _ = FileLogger::init(LogLevelFilter::Info, File::create("registration.log").unwrap());
    
    let config = load_configuration("");

    let db_conn = SqliteConnection::open(config.db_filename).unwrap();

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

    let mut chain = Chain::new(mount);
    chain.link_after(hbse);

    Iron::new(chain).http(config.socket_addr).unwrap();
}
