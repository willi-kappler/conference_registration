use iron::prelude::{Request, IronResult, Response};
use iron::status;

pub fn handle_main(req: &mut Request) -> IronResult<Response> {
    Ok(Response::with((status::Ok, "Hello World!")))
}
