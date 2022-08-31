use actix_web::{post, web, Responder, Result};
use serde::{Serialize, Deserialize};
use crate::err;

#[derive(Serialize, Deserialize)]
struct PostJob { // TODO return bad request
    source_code: String,
    language: String,
    user_id: i32,
    contest_id: i32,
    problem_id: i32,
}

fn get_err() -> Result<i32,err::Error> {
    //Err(std::io::Error::new(std::io::ErrorKind::AddrInUse, "An Error"))
    Err(err::Error::new(err::ErrorKind::ErrInternal, String::from("simpletest")))
}


#[post("/jobs")]
async fn post_jobs(body: web::Json<PostJob>) -> Result<impl Responder> {
    //let dir = tempdir::TempDir::new("oj")?;
    let dir = get_err()?;
    //let body = serde_json::from_str(body);
    Ok(body)
}