use std::collections::BTreeMap;

use iron::prelude::{Request, IronResult, Response, Set};
use iron::status;

use handlebars_iron::{Template};

use rustc_serialize::json::Json;

use params::{Params, Value, Map};

use plugin::Pluggable;


enum PriceCategory {
    Student,
    Regular
}

struct Registration {
    last_name: String,
    first_name: String,
    institutuion: String,
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
    let map = req.get_ref::<Params>().unwrap();
    
    let mut resp = Response::new();

    info!("handle_submit: {:?}", map);

    match map2Registration(map) {
        Some(registration) => insert_to_db(registration),
        None => info!("Registration invalid...")
    }
    
    let mut data : BTreeMap<String, Json> = BTreeMap::new();
    resp.set_mut(Template::new("submit", data)).set_mut(status::Ok);
    Ok(resp)
}

fn map2Registration(map: &Map) -> Option<Registration> {
    let mut result = Registration{
        last_name: "".to_string(),
        first_name: "".to_string(),
        institutuion: "".to_string(),
        street: "".to_string(),
        street_no: "".to_string(),
        zip_code: "".to_string(),
        city: "".to_string(),
        phone: "".to_string(),
        e_mail: "".to_string(),
        more_info: "".to_string(),
        price_category: PriceCategory::Student,
    };
    
    match map.find(&["last_name"]) {
        Some(&Value::String(ref last_name)) => result.last_name = last_name.to_string(),
        _ => return None
    }
    
    Some(result)
}

fn insert_to_db(registration: Registration) {
    // TODO
}
