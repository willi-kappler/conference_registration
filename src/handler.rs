use std::collections::BTreeMap;
use std::sync::{PoisonError, MutexGuard};
use std::net::{Ipv4Addr, AddrParseError};
use std::str::FromStr;

use iron::prelude::{Request, IronResult, Response, Set};
use iron::status;

use handlebars_iron::{Template};
use rustc_serialize::json::{Json, ToJson};
use params::{Params, Value, Map, ParamsError};
use plugin::Pluggable;
use persistent::{Read, Write, PersistentError};
use rusqlite::Connection;
use rusqlite;

use lettre::email::EmailBuilder;
use lettre::transport::smtp::{SecurityLevel, SmtpTransportBuilder};
use lettre::transport::smtp::authentication::Mechanism;
use lettre::transport::smtp::SUBMISSION_PORT;
use lettre::transport::EmailTransport;
use lettre;

use ::DBConnection;
use config::Configuration;


#[derive(Debug)]
pub enum HandleError {
    FormParameter,
    FormValue,
    Persistent,
    Mutex,
    SQL,
    Mail,
    SMTP,
    IP
}

impl From<PersistentError> for HandleError {
    fn from(_: PersistentError) -> HandleError {
        HandleError::Persistent
    }
}

impl From<ParamsError> for HandleError {
    fn from(_: ParamsError) -> HandleError {
        HandleError::FormParameter
    }
}

impl<'a> From<PoisonError<MutexGuard<'a, Connection>>> for HandleError {
    fn from(_: PoisonError<MutexGuard<'a, Connection>>) -> HandleError {
        HandleError::Mutex
    }
}

impl From<rusqlite::Error> for HandleError {
    fn from(_: rusqlite::Error) -> HandleError {
        HandleError::SQL
    }
}

impl From<lettre::email::error::Error> for HandleError {
    fn from(_: lettre::email::error::Error) -> HandleError {
        HandleError::Mail
    }
}

impl From<lettre::transport::smtp::error::Error> for HandleError {
    fn from(_: lettre::transport::smtp::error::Error) -> HandleError {
        HandleError::SMTP
    }
}

impl From<AddrParseError> for HandleError {
    fn from(_: AddrParseError) -> HandleError {
        HandleError::IP
    }
}


#[derive(Debug, PartialEq)]
enum PriceCategory {
    Student,
    Regular
}

#[derive(Debug, PartialEq)]
enum Title {
    Sir,
    Madam
}

#[derive(Debug, PartialEq)]
enum Course {
    Course1,
    Course2
}

#[derive(Debug, PartialEq)]
struct Registration {
    title: Title,
    last_name: String,
    first_name: String,
    institution: String,
    street: String,
    street_no: String,
    zip_code: String,
    city: String,
    phone: String,
    email_to: String,
    more_info: String,
    price_category: PriceCategory,
    course_type: Course
}


pub fn handle_main(req: &mut Request) -> IronResult<Response> {
    let map = req.get_ref::<Params>().unwrap();
    
    let mut resp = Response::new();

    info!("handle_main: {:?}", map);
    
    let data : BTreeMap<String, Json> = BTreeMap::new();
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
    
    try!(insert_to_db(&*db_connection, &registration));

    let config = try!(req.get::<Read<Configuration>>());
    
    try!(send_mail(&registration, &config));
    
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
        title: if try!(extract_string(&map, "title")) == "sir".to_string() { Title::Sir }
        else { Title::Madam },
        last_name: try!(extract_string(&map, "last_name")),
        first_name: try!(extract_string(&map, "first_name")),
        institution: try!(extract_string(&map, "institution")),
        street: try!(extract_string(&map, "street")),
        street_no: try!(extract_string(&map, "street_no")),
        zip_code: try!(extract_string(&map, "zip_code")),
        city: try!(extract_string(&map, "city")),
        phone: try!(extract_string(&map, "phone")),
        email_to: try!(extract_string(&map, "email_to")),
        more_info: try!(extract_string(&map, "more_info")),
        price_category: if try!(extract_string(&map, "price_category")) == "student".to_string() { PriceCategory::Student }
        else { PriceCategory::Regular },
        course_type: if try!(extract_string(&map, "course_type")) == "course1".to_string() { Course::Course1 }
        else { Course::Course2 }
    };

    Ok(result)
}

fn insert_to_db(db_connection: &Connection, registration: &Registration) -> Result<(), HandleError> {
    let title = if registration.title == Title::Sir { "sir".to_string() } else { "madam".to_string() };
    let price_category = if registration.price_category == PriceCategory::Student { "student".to_string() } else { "regular".to_string() };
    let course_type = if registration.course_type == Course::Course1 { "course1".to_string() } else { "course2".to_string() };
    
    try!(db_connection.execute("
         INSERT INTO registration (
           title,
           last_name,
           first_name,
           institution,
           street,
           street_no,
           zip_code,
           city,
           phone,
           email_to,
           more_info,
           price_category,
           course_type
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         ",&[
             &title,
             &registration.last_name,
             &registration.first_name,
             &registration.institution,
             &registration.street,
             &registration.street_no,
             &registration.zip_code,
             &registration.city,
             &registration.phone,
             &registration.email_to,
             &registration.more_info,
             &price_category,
             &course_type
         ]));

    
    Ok(())
}

fn send_mail(registration: &Registration, config: &Configuration) -> Result<(), HandleError> {
    let course = if registration.course_type == Course::Course1 { "3. März 2017" } else { "22. September 2017" };
    let subject = format!("Anmeldungsbestätigung: TGAG Fortbildung - {}", course);
    let greeting = if registration.title == Title::Sir { format!("Sehr geehrter Herr {},", registration.last_name) } else { format!("Sehr geehrte Frau {},", registration.last_name) };
    let price = if registration.price_category == PriceCategory::Student { "Student".to_string() } else { "Regulär".to_string() };
    let body = format!("{}\n Sie haben sich für den folgenden Kurs angemeldet:\n Zeitpunkt: {}\n Kategorie: {}\n\nMit freundlichen Grüßen,\ndie Fortbildungsorganisation", greeting, course, price);

    let email_to = registration.email_to.as_str();
    let email_from = config.email_from.as_str();
    
    let email = try!(EmailBuilder::new()
                    .to(email_to)
                    .from(email_from)
                    .body(&body)
                    .subject(&subject)
                    .build());

    let host_ip = try!(Ipv4Addr::from_str(&config.email_server));
    
    let mut mailer = try!(SmtpTransportBuilder::new((host_ip, SUBMISSION_PORT)))
        .hello_name(&config.email_hello)
        .credentials(&config.email_username, &config.email_password)
        .security_level(SecurityLevel::AlwaysEncrypt)
        .smtp_utf8(true)
        .authentication_mechanism(Mechanism::CramMd5)
        .connection_reuse(true).build();

    try!(mailer.send(email));
    
    Ok(())
}
