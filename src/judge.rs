use actix_web::{post, web, Responder, Result};
use serde::{Serialize, Deserialize};
use chrono;
use std::{
    fs, process::Command
};
use crate::{
    err, conf::Conf
};

#[derive(Serialize)]
#[allow(dead_code)]
enum State{
    Queueing,
    Running,
    Finished,
    Canceled,
}

#[derive(Serialize)]
#[allow(dead_code)]
enum CaseResult {
    Waiting,
    Running,
    Accepted,
    #[serde(rename = "Compilation Error")]
    CompilationError,
    #[serde(rename = "Compilation Success")]
    CompilationSuccess,
    #[serde(rename = "Wrong Answer")]
    WrongAnswer,
    #[serde(rename = "Runtime Error")]
    RuntimeError,
    #[serde(rename = "Time Limit Exeeded")]
    TimeLimitExceeded,
    #[serde(rename = "Memory Limit Exceeded")]
    MemoryLimitExceeded,
    #[serde(rename = "System Error")]
    SystemError,
    #[serde(rename = "SPJ Error")]
    SPJError
}

#[derive(Serialize)]
struct Case{
    id: i32,
    result: String,
    time: i32,
    memory: i32,
    info: String,
}

#[derive(Serialize, Deserialize)]
struct PostJob { // TODO return bad request
    source_code: String,
    language: String,
    user_id: i32,
    contest_id: i32,
    problem_id: i32,
}

#[derive(Serialize)]
struct PostJobRes {
    id: i32,
    created_time: String, //chrono::DateTime<chrono::Utc>
    updated_time: String,
    submission: PostJob,
    state: State,
    result: CaseResult,
    score: f64,
    cases: Vec<Case>,
}

impl PostJobRes {
    fn new(job: PostJob) -> Self {
        let time = chrono::Utc::now().to_string();
        Self {
            id: 0,
            created_time: time.clone(),
            updated_time: time,
            submission: job,
            state: State::Queueing,
            result: CaseResult::Waiting,
            score: 0f64,
            cases: vec![],
        }
    }
}

fn sleep(secs: u64, nanosecs: u32){
    use std::time::Duration;
    let sec = Duration::new(secs, nanosecs);
    std::thread::sleep(sec);
}

fn check_contest(conf: &Conf, job: &PostJob) -> Result<(), err::Error> {
    // TODO
    Ok(())
}
fn check_prob_id(conf: &Conf, id: i32) -> Result<(), err::Error> {
    for prob in conf.problems.iter() {
        if id == prob.id {
            return Ok(());
        }
    }
    Err(err::Error::new(err::ErrorKind::ErrNotFound, String::new()))
}
fn check_language(conf: &Conf, job: &PostJob) -> Result<(), err::Error> {
    for lang in conf.languages.iter() {
        if lang.name == job.language {
            return Ok(())
        }
    }
    Err(err::Error::new(err::ErrorKind::ErrNotFound, String::new()))
}

#[post("/jobs")]
async fn post_jobs(body: web::Json<PostJob>, conf: web::Data<Conf>) -> Result<impl Responder> {
    let job = body.into_inner();
    check_contest(&conf, &job)?;
    check_prob_id(&conf, job.problem_id)?;
    check_language(&conf, &job)?;

    let dir = tempdir::TempDir::new("oj")?;
    let file_path = dir.path().join("code.txt");
    fs::write(&file_path, &job.source_code)?;
    let _status = match job.language.as_str() {
        "Rust" => Command::new("rustc").arg(file_path.to_str().unwrap()).status()?,
        _ => unimplemented!()
    };
    log::info!("{:?}",chrono::Utc::now());

    let exe_path = dir.path().join("code").to_str().unwrap();

    let job = PostJobRes::new(job);

    Ok(web::Json(job))

}