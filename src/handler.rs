use std::collections::BTreeMap;
use std::sync::{PoisonError, MutexGuard};
use std::net::{Ipv4Addr, AddrParseError};
use std::str::FromStr;
use std::fmt;

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


#[derive(Debug, PartialEq)]
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
enum Title {
    Other,
    Msc,
    Dr,
    Prof
}

impl fmt::Display for Title {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            &Title::Msc => "msc",
            &Title::Dr => "dr",
            &Title::Prof => "prof",
            _ => "other"
        };

        write!(f, "{}", s)
    }
}

impl From<String> for Title {
    fn from(title: String) -> Title {
        if title == "msc" { Title::Msc }
        else if title == "dr" { Title::Dr }
        else if title == "prof" { Title::Prof }
        else { Title::Other }
    }
}

#[derive(Debug, PartialEq)]
enum Presentation {
    Poster,
    Talk,
    NotPresenting
}

impl fmt::Display for Presentation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            &Presentation::Poster => "poster",
            &Presentation::Talk => "talk",
            _ => "not_presenting"
        };

        write!(f, "{}", s)
    }
}

impl From<String> for Presentation {
    fn from(title: String) -> Presentation {
        if title == "poster" { Presentation::Poster }
        else if title == "talk" { Presentation::Talk }
        else { Presentation::NotPresenting }
    }
}

#[derive(Debug, PartialEq)]
enum Meal {
    MeatEater,
    Vegetarian,
}

impl fmt::Display for Meal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            &Meal::Vegetarian => "vegetarian",
            _ => "meat_eater"
        };

        write!(f, "{}", s)
    }
}

impl From<String> for Meal {
    fn from(title: String) -> Meal {
        if title == "vegetarian" { Meal::Vegetarian }
        else { Meal::MeatEater }
    }
}

#[derive(Debug, PartialEq)]
struct Registration {
    title: Title,
    last_name: String,
    first_name: String,
    email_to: String,
    institution: String,
    special_participant: bool,
    project_number: String,
    phd_student: bool,
    presentation: Presentation,
    presentation_title: String,
    meal_type: Meal,
    comment: String,
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
            message.insert("message".to_string(), "Your registration was successful.".to_json());
        }
        Err(e) => {
            error!("Error while processing data: {:?}", e);
            message.insert("message".to_string(), "An error occured. Please try it again later".to_json());
        }
    }

    let mut resp = Response::new();

    resp.set_mut(Template::new("submit", message)).set_mut(status::Ok);
    Ok(resp)
}

pub fn handle_login(req: &mut Request) -> IronResult<Response> {
    let map = req.get_ref::<Params>().unwrap();

    let mut resp = Response::new();

    info!("handle_login: {:?}", map);

    let data : BTreeMap<String, Json> = BTreeMap::new();
    resp.set_mut(Template::new("login", data)).set_mut(status::Ok);
    Ok(resp)
}

fn handle_form_data(req: &mut Request) -> Result<(), HandleError> {
    let map = try!(req.get::<Params>());

    info!("handle_submit: {:?}", map);

    let registration = try!(map2registration(map));

    let mutex = try!(req.get::<Write<DBConnection>>());

    let db_connection = try!(mutex.lock());

    try!(insert_into_db(&*db_connection, &registration));

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
        title: Title::from(try!(extract_string(&map, "title"))),
        last_name: try!(extract_string(&map, "last_name")),
        first_name: try!(extract_string(&map, "first_name")),
        email_to: try!(extract_string(&map, "email_to")),
        institution: try!(extract_string(&map, "institution")),
        special_participant: try!(extract_string(&map, "special_participant")) == "yes",
        project_number: try!(extract_string(&map, "project_number")),
        phd_student: try!(extract_string(&map, "phd_student")) == "yes",
        presentation: Presentation::from(try!(extract_string(&map, "presentation"))),
        presentation_title: try!(extract_string(&map, "presentation_title")),
        meal_type: Meal::from(try!(extract_string(&map, "meal_type"))),
        comment: try!(extract_string(&map, "comment"))
    };

    Ok(result)
}

fn insert_into_db(db_connection: &Connection, registration: &Registration) -> Result<(), HandleError> {
    try!(db_connection.execute("
         INSERT INTO registration (
           title,
           last_name,
           first_name,
           email_to,
           institution,
           special_participant,
           project_number,
           phd_student,
           presentation,
           presentation_title,
           meal_type,
           comment
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ",&[
             &(registration.title.to_string()),
             &registration.last_name,
             &registration.first_name,
             &registration.email_to,
             &registration.institution,
             &registration.special_participant,
             &registration.project_number,
             &registration.phd_student,
             &(registration.presentation.to_string()),
             &registration.presentation_title,
             &(registration.meal_type.to_string()),
             &registration.comment,
         ]));

    Ok(())
}

fn send_mail(registration: &Registration, config: &Configuration) -> Result<(), HandleError> {
    let subject = "Earthshape registration confirmation";
    let body = format!("Dear {} {},\nyou have sucessfully registered for the Earthshape meeting from 28 March to 31 March 2017.\n\nBest regards,\nthe Earthshape organisation team", registration.first_name, registration.last_name);

    let email_to = registration.email_to.as_str();
    let email_from = config.email_from.as_str();

    let email = try!(EmailBuilder::new()
                    .to(email_to)
                    .from(email_from)
                    .cc(email_from)
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

#[cfg(test)]
mod tests {
    use super::{extract_string, map2registration, insert_into_db, send_mail,
        Registration, HandleError, Title, Presentation, Meal};
    use config::{load_configuration};
    use params::{Value, Map};

    use rusqlite::Connection;


    #[test]
    fn test_extract_string() {
        let mut map = Map::new();
        map.assign("name", Value::String("Bob".into())).unwrap();
        let result = extract_string(&map, "name").unwrap();

        assert_eq!(result, "Bob".to_string());
    }

    #[test]
    fn test_map2registration1() {
        let mut map = Map::new();
        map.assign("title", Value::String("other".into())).unwrap();
        map.assign("last_name", Value::String("Smith".into())).unwrap();
        map.assign("first_name", Value::String("Bob".into())).unwrap();
        map.assign("email_to", Value::String("bob@smith.com".into())).unwrap();
        map.assign("institution", Value::String("Some university".into())).unwrap();
        map.assign("special_participant", Value::String("yes".into())).unwrap();
        map.assign("project_number", Value::String("3b".into())).unwrap();
        map.assign("phd_student", Value::String("no".into())).unwrap();
        map.assign("presentation", Value::String("talk".into())).unwrap();
        map.assign("presentation_title", Value::String("how to get rich".into())).unwrap();
        map.assign("meal_type", Value::String("vegetarian".into())).unwrap();
        map.assign("comment", Value::String("pure awsomeness".into())).unwrap();

        let result = map2registration(map).unwrap();
        let expected = Registration{
            title: Title::Other,
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            special_participant: true,
            project_number: "3b".to_string(),
            phd_student: false,
            presentation: Presentation::Talk,
            presentation_title: "how to get rich".to_string(),
            meal_type: Meal::Vegetarian,
            comment: "pure awsomeness".to_string()
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_map2registration2() {
        let mut map = Map::new();
        map.assign("title", Value::String("msc".into())).unwrap();
        map.assign("last_name", Value::String("Smith".into())).unwrap();
        map.assign("first_name", Value::String("Bob".into())).unwrap();
        map.assign("email_to", Value::String("bob@smith.com".into())).unwrap();
        map.assign("institution", Value::String("Some university".into())).unwrap();
        map.assign("special_participant", Value::String("yes".into())).unwrap();
        map.assign("project_number", Value::String("3b".into())).unwrap();
        map.assign("phd_student", Value::String("yes".into())).unwrap();
        map.assign("presentation", Value::String("talk".into())).unwrap();
        map.assign("presentation_title", Value::String("how to get rich".into())).unwrap();
        map.assign("meal_type", Value::String("vegetarian".into())).unwrap();
        map.assign("comment", Value::String("pure awsomeness".into())).unwrap();

        let result = map2registration(map).unwrap();
        let expected = Registration{
            title: Title::Msc,
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            special_participant: true,
            project_number: "3b".to_string(),
            phd_student: true,
            presentation: Presentation::Talk,
            presentation_title: "how to get rich".to_string(),
            meal_type: Meal::Vegetarian,
            comment: "pure awsomeness".to_string()
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_map2registration3() {
        let mut map = Map::new();
        map.assign("title", Value::String("prof".into())).unwrap();
        map.assign("last_name", Value::String("Smith".into())).unwrap();
        map.assign("first_name", Value::String("Bob".into())).unwrap();
        map.assign("email_to", Value::String("bob@smith.com".into())).unwrap();
        map.assign("institution", Value::String("Some university".into())).unwrap();
        map.assign("special_participant", Value::String("no".into())).unwrap();
        map.assign("project_number", Value::String("3b".into())).unwrap();
        map.assign("phd_student", Value::String("no".into())).unwrap();
        map.assign("presentation", Value::String("not_presenting".into())).unwrap();
        map.assign("presentation_title", Value::String("how to get rich".into())).unwrap();
        map.assign("meal_type", Value::String("meat_eater".into())).unwrap();
        map.assign("comment", Value::String("pure awsomeness".into())).unwrap();

        let result = map2registration(map).unwrap();
        let expected = Registration{
            title: Title::Prof,
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            special_participant: false,
            project_number: "3b".to_string(),
            phd_student: false,
            presentation: Presentation::NotPresenting,
            presentation_title: "how to get rich".to_string(),
            meal_type: Meal::MeatEater,
            comment: "pure awsomeness".to_string()
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_map2registration4() {
        let mut map = Map::new();
        map.assign("title", Value::String("dr".into())).unwrap();
        map.assign("last_name", Value::String("Smith".into())).unwrap();
        map.assign("first_name", Value::String("Bob".into())).unwrap();
        map.assign("email_to", Value::String("bob@smith.com".into())).unwrap();
        map.assign("institution", Value::String("Some university".into())).unwrap();
        map.assign("special_participant", Value::String("yes".into())).unwrap();
        map.assign("project_number", Value::String("3b".into())).unwrap();
        map.assign("phd_student", Value::String("no".into())).unwrap();
        map.assign("presentation", Value::String("poster".into())).unwrap();
        map.assign("presentation_title", Value::String("how to get rich".into())).unwrap();
        map.assign("meal_type", Value::String("vegetarian".into())).unwrap();
        map.assign("comment", Value::String("pure awsomeness".into())).unwrap();

        let result = map2registration(map).unwrap();
        let expected = Registration{
            title: Title::Dr,
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            special_participant: true,
            project_number: "3b".to_string(),
            phd_student: false,
            presentation: Presentation::Poster,
            presentation_title: "how to get rich".to_string(),
            meal_type: Meal::Vegetarian,
            comment: "pure awsomeness".to_string()
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_insert_into_db1() {
        let conn = Connection::open_in_memory().unwrap();
        let reg = Registration {
            title: Title::Other,
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            special_participant: true,
            project_number: "3b".to_string(),
            phd_student: false,
            presentation: Presentation::Talk,
            presentation_title: "how to get rich".to_string(),
            meal_type: Meal::Vegetarian,
            comment: "pure awsomeness".to_string()
        };

        conn.execute("CREATE TABLE registration (
                  id              INTEGER PRIMARY KEY,
                  title           TEXT NOT NULL,
                  last_name       TEXT NOT NULL,
                  first_name      TEXT NOT NULL,
                  email_to        TEXT NOT NULL,
                  institution     TEXT NOT NULL,
                  special_participant TEXT NOT NULL,
                  project_number  TEXT NOT NULL,
                  phd_student     TEXT NOT NULL,
                  presentation    TEXT NOT NULL,
                  presentation_title TEXT NOT NULL,
                  meal_type       TEXT NOT NULL,
                  comment         TEXT NOT NULL
                  )", &[]).unwrap();

        assert!(insert_into_db(&conn, &reg).is_ok());

        let mut stmt = conn.prepare("SELECT * FROM registration").unwrap();
        let mut rows = stmt.query(&[]).unwrap();
        let result = rows.next().unwrap().unwrap();

        assert_eq!(result.get::<i32, i32>(0), 1);
        assert_eq!(result.get::<i32, String>(1), "other");
        assert_eq!(result.get::<i32, String>(2), "Smith");
        assert_eq!(result.get::<i32, String>(3), "Bob");
        assert_eq!(result.get::<i32, String>(4), "bob@smith.com");
        assert_eq!(result.get::<i32, String>(5), "Some university");
        assert_eq!(result.get::<i32, String>(6), "1");
        assert_eq!(result.get::<i32, String>(7), "3b");
        assert_eq!(result.get::<i32, String>(8), "0");
        assert_eq!(result.get::<i32, String>(9), "talk");
        assert_eq!(result.get::<i32, String>(10), "how to get rich");
        assert_eq!(result.get::<i32, String>(11), "vegetarian");
        assert_eq!(result.get::<i32, String>(12), "pure awsomeness");
    }

    #[test]
    fn test_insert_into_db2() {
        let conn = Connection::open("registration_database.sqlite3");
        assert!(conn.is_ok());
        let conn = conn.unwrap();

        let reg = Registration {
            title: Title::Other,
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            special_participant: false,
            project_number: "7a".to_string(),
            phd_student: true,
            presentation: Presentation::Talk,
            presentation_title: "how to get rich".to_string(),
            meal_type: Meal::Vegetarian,
            comment: "pure awsomeness".to_string()
        };

        assert!(insert_into_db(&conn, &reg).is_ok());

        let stmt = conn.prepare("SELECT * FROM registration WHERE id = '1'");
        assert!(stmt.is_ok());
        let mut stmt = stmt.unwrap();

        let rows = stmt.query(&[]);
        assert!(rows.is_ok());
        let mut rows = rows.unwrap();

        let result = rows.next();
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.is_ok());
        let result = result.unwrap();


        assert_eq!(result.get::<i32, i32>(0), 1);
        assert_eq!(result.get::<i32, String>(1), "other");
        assert_eq!(result.get::<i32, String>(2), "Smith");
        assert_eq!(result.get::<i32, String>(3), "Bob");
        assert_eq!(result.get::<i32, String>(4), "bob@smith.com");
        assert_eq!(result.get::<i32, String>(5), "Some university");
        assert_eq!(result.get::<i32, String>(6), "0");
        assert_eq!(result.get::<i32, String>(7), "7a");
        assert_eq!(result.get::<i32, String>(8), "1");
        assert_eq!(result.get::<i32, String>(9), "talk");
        assert_eq!(result.get::<i32, String>(10), "how to get rich");
        assert_eq!(result.get::<i32, String>(11), "vegetarian");
        assert_eq!(result.get::<i32, String>(12), "pure awsomeness");

        conn.execute("DELETE FROM registration WHERE id = '1';", &[]).unwrap();
    }

    #[test]
    fn test_send_mail1() {
        let config = load_configuration("test_config2.ini").unwrap();

        let reg = Registration {
            title: Title::Other,
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            special_participant: true,
            project_number: "3b".to_string(),
            phd_student: false,
            presentation: Presentation::Talk,
            presentation_title: "how to get rich".to_string(),
            meal_type: Meal::Vegetarian,
            comment: "pure awsomeness".to_string()
        };

        let result = send_mail(&reg, &config);

        assert_eq!(result, Err(HandleError::SMTP));
    }

    #[test]
    fn test_send_mail2() {
        let config = load_configuration("test_config2.ini").unwrap();

        let reg = Registration {
            title: Title::Other,
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            special_participant: true,
            project_number: "3b".to_string(),
            phd_student: false,
            presentation: Presentation::Talk,
            presentation_title: "how to get rich".to_string(),
            meal_type: Meal::Vegetarian,
            comment: "pure awsomeness".to_string()
        };

        let result = send_mail(&reg, &config);

        assert_eq!(result, Err(HandleError::SMTP));
    }


}
