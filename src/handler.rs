use std::collections::BTreeMap;

use iron::prelude::{Request, IronResult, Response, Set};
use iron::status;

use handlebars_iron::{Template};

use rustc_serialize::json::{Json, ToJson};

use params::{Params, Value, Map, ParamsError};

use plugin::Pluggable;

use persistent::{Write, PersistentError};

use rusqlite::Connection;

use std::sync::{PoisonError, MutexGuard};

use ::DBConnection;


#[derive(Debug)]
pub enum HandleError {
    FormParameter,
    FormValue,
    Persistent,
    Mutex,
    DBSend
}

impl From<PersistentError> for HandleError {
    fn from(err: PersistentError) -> HandleError {
        HandleError::Persistent
    }
}

impl From<ParamsError> for HandleError {
    fn from(err: ParamsError) -> HandleError {
        HandleError::FormParameter
    }
}

impl<'a> From<PoisonError<MutexGuard<'a, Connection>>> for HandleError {
    fn from(err: PoisonError<MutexGuard<'a, Connection>>) -> HandleError {
        HandleError::Mutex
    }
}

enum PriceCategory {
    Student,
    Regular
}

struct Registration {
    last_name: String,
    first_name: String,
    institution: String,
    street: String,
    street_no: String,
    zip_code: String,
    city: String,
    phone: String,
    e_mail: String,
    more_info: String,
    price_category: PriceCategory
}


pub fn handle_main(req: &mut Request) -> IronResult<Response> {
    let map = req.get_ref::<Params>().unwrap();
    
    let mut resp = Response::new();

    info!("handle_main: {:?}", map);
    
    let mut data : BTreeMap<String, Json> = BTreeMap::new();
    resp.set_mut(Template::new("index", data)).set_mut(status::Ok);
    Ok(resp)
}

pub fn handle_submit(req: &mut Request) -> IronResult<Response> {
    let mut message: BTreeMap<String, Json> = BTreeMap::new();

    match handle_form_data(req) {
        Ok(_) => {
            info!("Data handled successfully");
            message.insert("message".to_string(), "Ihre Anmeldung war erfolgreich".to_json());
        }
        Err(_) => {
            error!("Error while processing data");
            message.insert("message".to_string(), "Ein Fehler ist aufgetreten. Bitte versuchen Sie es später noch einmal.".to_json());
        }
    }
    
    let mut resp = Response::new();

    resp.set_mut(Template::new("submit", message)).set_mut(status::Ok);
    Ok(resp)
}

fn handle_form_data(req: &mut Request) -> Result<(), HandleError> {
    let map = try!(req.get::<Params>());
    
    info!("handle_submit: {:?}", map);

    let registration = try!(map2registration(map));

    let mutex = try!(req.get::<Write<DBConnection>>());

    let db_connection = try!(mutex.lock());
    
    try!(insert_to_db(&*db_connection, registration));

    Ok(())
}

fn extract_string(map: &Map, key: &str) -> Result<String, HandleError> {
    match map.find(&[key]) {
        Some(&Value::String(ref value)) => Ok(value.to_string()),
        _ => Err(HandleError::FormValue)
    }
}

fn map2registration(map: Map) -> Result<Registration, HandleError> {
    let result = Registration{
        last_name: try!(extract_string(&map, "last_name")),
        first_name: try!(extract_string(&map, "first_name")),
        institution: try!(extract_string(&map, "institution")),
        street: try!(extract_string(&map, "street")),
        street_no: try!(extract_string(&map, "street_no")),
        zip_code: try!(extract_string(&map, "zip_code")),
        city: try!(extract_string(&map, "city")),
        phone: try!(extract_string(&map, "phone")),
        e_mail: try!(extract_string(&map, "e_mail")),
        more_info: try!(extract_string(&map, "more_info")),
        price_category: if try!(extract_string(&map, "price_category")) == "student".to_string() { PriceCategory::Student }
        else { PriceCategory::Regular }
    };

    Ok(result)
}

fn insert_to_db(db_connection: &Connection, registration: Registration) -> Result<(), HandleError> {
    // TODO
    Ok(())
}
