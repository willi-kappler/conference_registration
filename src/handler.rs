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
use oven::prelude::{ResponseExt, RequestExt};
use cookie;

use ::DBConnection;
use config::Configuration;
use chrono::Local;

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
    NoMeal
}

impl fmt::Display for Meal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            &Meal::Vegetarian => "vegetarian",
            &Meal::MeatEater => "meat_eater",
            &Meal::NoMeal => "no_meal"
        };

        write!(f, "{}", s)
    }
}

impl From<String> for Meal {
    fn from(title: String) -> Meal {
        if title == "vegetarian" { Meal::Vegetarian }
        else if title == "meat_eater" { Meal::MeatEater }
        else { Meal::NoMeal }
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
    pay_cash: bool,
    comment: String,
}

fn check_login(req: &mut Request) -> Result<bool, HandleError> {
    let map = req.get::<Params>()?;

    info!("{}: handle_submit: {:?}", Local::now().format("%Y.%m.%d"), map);

    let config = req.get::<Read<Configuration>>()?;

    let username = extract_string(&map, "username")?;
    let password = extract_string(&map, "password")?;

    Ok(username == config.login_user && password == config.login_passwd)
}

pub fn handle_main(req: &mut Request) -> IronResult<Response> {
    let local_time = Local::now().format("%Y.%m.%d");
    let mut message: BTreeMap<String, Json> = BTreeMap::new();
    let mut resp = Response::new();

    info!("{}: handle_main", local_time);

    match check_login(req) {
        Ok(login_successful) => {
            if login_successful {
                resp.set_mut(Template::new("main", message)).set_mut(status::Ok);

                let mut cookie = cookie::Cookie::new("login".to_string(), "success".to_string());
                cookie.max_age = Some(60 * 60); // 60 * 60 seconds = 3600 seconds = 1 hour
                cookie.secure = false; // Also allow to send cookie when connection is not secure
                resp.set_cookie(cookie);
            } else {
                message.insert("message".to_string(), "Wrong user name or password!".to_json());
                resp.set_mut(Template::new("login", message)).set_mut(status::Ok);

                let mut cookie = cookie::Cookie::new("login".to_string(), "fail".to_string());
                cookie.max_age = Some(60 * 60); // 60 * 60 seconds = 3600 seconds = 1 hour
                cookie.secure = false;
                resp.set_cookie(cookie);
            }
        }
        Err(e) => {
            if e == HandleError::FormValue {
                let login_cookie = req.get_cookie("login");

                if let Some(stored_cookie) = login_cookie {
                    if stored_cookie.value == "success" {
                        resp.set_mut(Template::new("main", message)).set_mut(status::Ok);
                    } else {
                        message.insert("message".to_string(), "Please log in first!".to_json());
                        resp.set_mut(Template::new("login", message)).set_mut(status::Ok);
                    }
                } else {
                    message.insert("message".to_string(), "Please log in first!".to_json());
                    resp.set_mut(Template::new("login", message)).set_mut(status::Ok);
                }
            } else {
                error!("{}: Error while processing data: {:?}", local_time, e);
                message.insert("message".to_string(), "An error occured. Please try it again later".to_json());
                resp.set_mut(Template::new("login", message)).set_mut(status::Ok);
            }
        }
    }

    Ok(resp)
}

fn get_cookie(req: &mut Request) -> Option<cookie::Cookie> {
    let cookie = req.get_cookie("login");
    match cookie {
        Some(cookie) => Some(cookie.clone()),
        None => None
    }
}

pub fn handle_submit(req: &mut Request) -> IronResult<Response> {
    let mut message: BTreeMap<String, Json> = BTreeMap::new();
    let mut resp = Response::new();

    let login_cookie = get_cookie(req);

    if let Some(stored_cookie) = login_cookie {
        if stored_cookie.value == "success" {
            let local_time = Local::now().format("%Y.%m.%d");

            match handle_form_data(req) {
                Ok(_) => {
                    info!("{}: Data handled successfully", local_time);
                    message.insert("message".to_string(), "Your registration was successful. You should receive a confirmation e-mail. (Please also check your spam folder)".to_json());
                }
                Err(e) => {
                    error!("{}: Error while processing data: {:?}", local_time, e);
                    message.insert("message".to_string(), "An error occured. Please try it again later".to_json());
                }
            }

            resp.set_mut(Template::new("submit", message)).set_mut(status::Ok);
        } else {
            message.insert("message".to_string(), "Please log in first!".to_json());
            resp.set_mut(Template::new("login", message)).set_mut(status::Ok);
        }
    } else {
        message.insert("message".to_string(), "Please log in first!".to_json());
        resp.set_mut(Template::new("login", message)).set_mut(status::Ok);
    }

    Ok(resp)
}

pub fn handle_login(req: &mut Request) -> IronResult<Response> {
    let mut message: BTreeMap<String, Json> = BTreeMap::new();
    let mut resp = Response::new();

    let login_cookie = get_cookie(req);
    let map = req.get_ref::<Params>().unwrap();

    info!("{}: handle_login: {:?}", Local::now().format("%Y.%m.%d"), map);

    if let Some(stored_cookie) = login_cookie {
        if stored_cookie.value == "success" {
            resp.set_mut(Template::new("main", message)).set_mut(status::Ok);
        } else {
            message.insert("message".to_string(), "Please log in first!".to_json());
            resp.set_mut(Template::new("login", message)).set_mut(status::Ok);
        }
    } else {
        message.insert("message".to_string(), "Please log in first!".to_json());
        resp.set_mut(Template::new("login", message)).set_mut(status::Ok);
    }

    Ok(resp)
}

fn handle_form_data(req: &mut Request) -> Result<(), HandleError> {
    let map = req.get::<Params>()?;

    info!("{}: handle_submit: {:?}", Local::now().format("%Y.%m.%d"), map);

    let registration = map2registration(map)?;

    let mutex = req.get::<Write<DBConnection>>()?;

    let db_connection = mutex.lock()?;

    insert_into_db(&*db_connection, &registration)?;

    let config = req.get::<Read<Configuration>>()?;

    send_mail(&registration, &config)?;

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
        title: Title::from(extract_string(&map, "title")?),
        last_name: extract_string(&map, "last_name")?,
        first_name: extract_string(&map, "first_name")?,
        email_to: extract_string(&map, "email_to")?,
        institution: extract_string(&map, "institution")?,
        special_participant: extract_string(&map, "special_participant")? == "yes",
        project_number: extract_string(&map, "project_number")?,
        phd_student: extract_string(&map, "phd_student")? == "yes",
        presentation: Presentation::from(extract_string(&map, "presentation")?),
        presentation_title: extract_string(&map, "presentation_title")?,
        meal_type: Meal::from(extract_string(&map, "meal_type")?),
        pay_cash: extract_string(&map, "pay_cash")? == "yes",
        comment: extract_string(&map, "comment")?
    };

    Ok(result)
}

fn insert_into_db(db_connection: &Connection, registration: &Registration) -> Result<(), HandleError> {
    db_connection.execute("
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
       pay_cash,
       comment
   ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
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
         &registration.pay_cash,
         &registration.comment,
     ])?;

    Ok(())
}

fn send_mail(registration: &Registration, config: &Configuration) -> Result<(), HandleError> {
    let subject = "Earthshape registration confirmation";
    let body = format!("Dear {} {},\nyou have sucessfully registered for the Earthshape meeting from 28 March to 31 March 2017.\n\nBest regards,\nthe Earthshape organisation team", registration.first_name, registration.last_name);

    let email_to = registration.email_to.as_str();
    let email_from = config.email_from.as_str();

    let email = EmailBuilder::new()
                .to(email_to)
                .from(email_from)
                //.cc(email_from)
                .body(&body)
                .subject(&subject)
                .build()?;

    let host_ip = Ipv4Addr::from_str(&config.email_server)?;

    let mut mailer = SmtpTransportBuilder::new((host_ip, SUBMISSION_PORT))?
        .hello_name(&config.email_hello)
        .credentials(&config.email_username, &config.email_password)
        .security_level(SecurityLevel::AlwaysEncrypt)
        .smtp_utf8(true)
        .authentication_mechanism(Mechanism::CramMd5)
        .connection_reuse(true).build();

    mailer.send(email)?;

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
        map.assign("pay_cash", Value::String("yes".into())).unwrap();
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
            pay_cash: true,
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
        map.assign("pay_cash", Value::String("no".into())).unwrap();
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
            pay_cash: false,
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
        map.assign("pay_cash", Value::String("yes".into())).unwrap();
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
            pay_cash: true,
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
        map.assign("pay_cash", Value::String("no".into())).unwrap();
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
            pay_cash: false,
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
            pay_cash: true,
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
                  pay_cash        TEXT NOT NULL,
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
        assert_eq!(result.get::<i32, String>(12), "1");
        assert_eq!(result.get::<i32, String>(13), "pure awsomeness");
    }

    #[test]
    fn test_insert_into_db2() {
        let conn = Connection::open("registration_database.sqlite3");
        assert!(conn.is_ok());
        let conn = conn.unwrap();

        conn.execute("DELETE FROM registration;", &[]).unwrap();

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
            pay_cash: false,
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
        assert_eq!(result.get::<i32, String>(13), "0");

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
            pay_cash: true,
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
            pay_cash: false,
            comment: "pure awsomeness".to_string()
        };

        let result = send_mail(&reg, &config);

        assert_eq!(result, Err(HandleError::SMTP));
    }


}
