use std::collections::BTreeMap;
use std::sync::{PoisonError, MutexGuard};
use std::net::{Ipv4Addr, AddrParseError};
use std::str::FromStr;

use iron::prelude::{Request, IronResult, Response, Set};
use iron::status;

use handlebars_iron::{Template};
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


// Name
// email
// Origanization/company
// student / non-student
// one sentence explanation of your activities in this field.


#[derive(Debug, PartialEq)]
struct Registration {
    last_name: String,
    first_name: String,
    email_to: String,
    institution: String,
    student: bool,
    more_info: String,
}


pub fn handle_main(req: &mut Request) -> IronResult<Response> {
    let map = req.get_ref::<Params>().unwrap();

    let mut resp = Response::new();

    info!("handle_main: {:?}", map);

    let data: BTreeMap<String, String> = BTreeMap::new();
    resp.set_mut(Template::new("index", data)).set_mut(status::Ok);
    Ok(resp)
}

pub fn handle_submit(req: &mut Request) -> IronResult<Response> {
    let mut message = BTreeMap::new();

    match handle_form_data(req) {
        Ok(_) => {
            info!("Data handled successfully");
            message.insert("message".to_string(), "Ihre Anmeldung war erfolgreich".to_string());
        }
        Err(e) => {
            error!("Error while processing data: {:?}", e);
            message.insert("message".to_string(), "Ein Fehler ist aufgetreten. Bitte versuchen Sie es später noch einmal.".to_string());
        }
    }

    let mut resp = Response::new();

    resp.set_mut(Template::new("submit", message)).set_mut(status::Ok);
    Ok(resp)
}

fn handle_form_data(req: &mut Request) -> Result<(), HandleError> {
    let map = req.get::<Params>()?;

    info!("handle_submit: {:?}", map);

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

fn extract_bool(map: &Map, key: &str) -> Result<bool, HandleError> {
    let value = extract_string(map, key)?;

    match value.as_ref() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(HandleError::FormValue)
    }
}

fn map2registration(map: Map) -> Result<Registration, HandleError> {
    let result = Registration{
        last_name: extract_string(&map, "last_name")?,
        first_name: extract_string(&map, "first_name")?,
        email_to: extract_string(&map, "email_to")?,
        institution: extract_string(&map, "institution")?,
        student: extract_bool(&map, "student")?,
        more_info: extract_string(&map, "more_info")?,
    };

    Ok(result)
}

fn create_db_table(db_connection: &Connection) -> Result<i32, rusqlite::Error> {
    db_connection.execute("CREATE TABLE registration (
      id              INTEGER PRIMARY KEY,
      last_name       TEXT NOT NULL,
      first_name      TEXT NOT NULL,
      email_to        TEXT NOT NULL,
      institution     TEXT NOT NULL,
      student         TEXT NOT NULL,
      more_info       TEXT NOT NULL
    );", &[])
}

fn insert_into_db(db_connection: &Connection, registration: &Registration) -> Result<(), HandleError> {
    db_connection.execute("
         INSERT INTO registration (
           last_name,
           first_name,
           email_to,
           institution,
           student,
           more_info
       ) VALUES ($1, $2, $3, $4, $5, $6);
         ",&[
             &registration.last_name,
             &registration.first_name,
             &registration.email_to,
             &registration.institution,
             &registration.student,
             &registration.more_info
         ])?;


    Ok(())
}

fn send_mail(registration: &Registration, config: &Configuration) -> Result<(), HandleError> {
    let subject = "Registration for Leopoldina";
    let body = format!("Dear {} {},\n\nYou have successfully registered for the Leopoldina meeting. Best regards,\nthe organisation team", registration.first_name, registration.last_name);

    let email_to = registration.email_to.as_str();
    let email_from = config.email_from.as_str();

    let email = EmailBuilder::new()
                    .to(email_to)
                    .from(email_from)
                    .cc(email_from)
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
    use super::{extract_string, extract_bool, map2registration, insert_into_db, send_mail,
        Registration, create_db_table};
    use config::{load_configuration};
    use params::{Value, Map};

    use rusqlite::Connection;

    use std::fs;

    #[test]
    fn test_extract_string() {
        let mut map = Map::new();
        map.assign("name", Value::String("Bob".into())).unwrap();
        let result = extract_string(&map, "name").unwrap();

        assert_eq!(result, "Bob".to_string());
    }

    #[test]
    fn test_extract_bool1() {
        let mut map = Map::new();
        map.assign("student", Value::String("yes".into())).unwrap();
        let result = extract_bool(&map, "student").unwrap();

        assert_eq!(result, true);
    }

    #[test]
    fn test_extract_bool2() {
        let mut map = Map::new();
        map.assign("student", Value::String("no".into())).unwrap();
        let result = extract_bool(&map, "student").unwrap();

        assert_eq!(result, false);
    }

    #[test]
    fn test_map2registration1() {
        let mut map = Map::new();
        map.assign("last_name", Value::String("Smith".into())).unwrap();
        map.assign("first_name", Value::String("Bob".into())).unwrap();
        map.assign("email_to", Value::String("bob@smith.com".into())).unwrap();
        map.assign("institution", Value::String("Some university".into())).unwrap();
        map.assign("student", Value::String("no".into())).unwrap();
        map.assign("more_info", Value::String("Some more information".into())).unwrap();

        let result = map2registration(map).unwrap();
        let expected = Registration{
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            student: false,
            more_info: "Some more information".to_string()
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_map2registration2() {
        let mut map = Map::new();
        map.assign("last_name", Value::String("Smith".into())).unwrap();
        map.assign("first_name", Value::String("Bob".into())).unwrap();
        map.assign("email_to", Value::String("bob@smith.com".into())).unwrap();
        map.assign("institution", Value::String("Some university".into())).unwrap();
        map.assign("student", Value::String("yes".into())).unwrap();
        map.assign("more_info", Value::String("Some more information".into())).unwrap();

        let result = map2registration(map).unwrap();
        let expected = Registration{
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            student: true,
            more_info: "Some more information".to_string()
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_insert_into_db1() {
        let conn = Connection::open_in_memory().unwrap();
        let reg = Registration {
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob.smith@somewhere.com".to_string(),
            institution: "Some university".to_string(),
            student: true,
            more_info: "Some more information".to_string()
        };

        create_db_table(&conn).unwrap();

        assert!(insert_into_db(&conn, &reg).is_ok());

        let mut stmt = conn.prepare("SELECT * FROM registration;").unwrap();
        let mut rows = stmt.query(&[]).unwrap();
        let result = rows.next().unwrap().unwrap();

        assert_eq!(result.get::<i32, i32>(0), 1);
        assert_eq!(result.get::<i32, String>(1), "Smith");
        assert_eq!(result.get::<i32, String>(2), "Bob");
        assert_eq!(result.get::<i32, String>(3), "bob.smith@somewhere.com");
        assert_eq!(result.get::<i32, String>(4), "Some university");
        assert_eq!(result.get::<i32, String>(5), "1");
        assert_eq!(result.get::<i32, String>(6), "Some more information");
    }

    #[test]
    fn test_insert_into_db2() {
        let file_name = "registration_database.sqlite3";

        // Remove sqlite file if it already exists
        let _ = fs::remove_file(file_name);

        let conn = Connection::open(file_name).unwrap();

        let reg = Registration {
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob.smith@somewhere.com".to_string(),
            institution: "Some university".to_string(),
            student: false,
            more_info: "Some more information".to_string()
        };

        create_db_table(&conn).unwrap();

        assert!(insert_into_db(&conn, &reg).is_ok());

        let mut stmt = conn.prepare("SELECT * FROM registration WHERE last_name = 'Smith';").unwrap();
        let mut rows = stmt.query(&[]).unwrap();
        let result = rows.next().unwrap().unwrap();

        assert_eq!(result.get::<i32, String>(1), "Smith");
        assert_eq!(result.get::<i32, String>(2), "Bob");
        assert_eq!(result.get::<i32, String>(3), "bob.smith@somewhere.com");
        assert_eq!(result.get::<i32, String>(4), "Some university");
        assert_eq!(result.get::<i32, String>(5), "0");
        assert_eq!(result.get::<i32, String>(6), "Some more information");

        conn.execute("DELETE FROM registration WHERE last_name = 'Smith';", &[]).unwrap();

        fs::remove_file(file_name).unwrap();
    }

    #[test]
    fn test_send_mail1() {
        let config = load_configuration("test_config2.ini").unwrap();

        let reg = Registration {
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            student: false,
            more_info: "Some more information".to_string()
        };

        let result = send_mail(&reg, &config);

        assert!(result.is_ok());
    }

    #[test]
    fn test_send_mail2() {
        let config = load_configuration("test_config2.ini").unwrap();

        let reg = Registration {
            last_name: "Smith".to_string(),
            first_name: "Bob".to_string(),
            email_to: "bob@smith.com".to_string(),
            institution: "Some university".to_string(),
            student: true,
            more_info: "Some more information".to_string()
        };

        let result = send_mail(&reg, &config);

        assert!(result.is_ok());
    }


}
