use std::collections::BTreeMap;

use iron::prelude::{Request, IronResult, Response, Set};
use iron::status;

use handlebars_iron::{Template};

use rustc_serialize::json::Json;

pub fn handle_main(req: &mut Request) -> IronResult<Response> {
    let mut resp = Response::new();

    let mut data : BTreeMap<String, Json> = BTreeMap::new();
    resp.set_mut(Template::new("index", data)).set_mut(status::Ok);
    Ok(resp)
}
