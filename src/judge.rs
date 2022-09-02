use crate::{
    conf::{Conf, Problem, ProblemType},
    err,
};
use actix_web::http::StatusCode;
use actix_web::{post, web, HttpResponse, Responder, Result};
use chrono;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    process::{Command, Stdio},
    time,
};
use wait_timeout::ChildExt;

#[derive(Serialize)]
#[allow(dead_code)]
enum State {
    Queueing,
    Running,
    Finished,
    Canceled,
}

#[derive(Clone, Serialize, PartialEq)]
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
    SPJError,
}

impl CaseResult {
    fn priority(&self) -> u32 {
        match *self {
            Self::Accepted => 1,
            Self::WrongAnswer => 2,
            Self::RuntimeError => 3,
            Self::TimeLimitExceeded => 4,
            Self::CompilationError => 5,
            _ => unreachable!(),
        }
    }
}

#[derive(Serialize)]
struct Case {
    id: i32,
    result: CaseResult,
    time: u64,
    memory: i32,
    info: String,
}

#[derive(Serialize, Deserialize)]
struct PostJob {
    // TODO return bad request
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

    fn load_cases(&mut self, cases: Vec<Case>, prob: &Problem) {
        let mut result = CaseResult::Accepted;
        let mut score = 0f64;
        for (case_res, case_cfg) in cases.iter().zip(prob.cases.iter()) {
            if result.priority() < case_res.result.priority() {
                result = case_res.result.clone();
            }
            if case_res.result == CaseResult::Accepted {
                score += case_cfg.score;
            }
        }
        self.state = State::Finished;
        self.result = result;
        self.score = score;
        self.cases = cases;
    }
}

fn sleep(secs: u64, nanosecs: u32) {
    use time::Duration;
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
            return Ok(());
        }
    }
    Err(err::Error::new(err::ErrorKind::ErrNotFound, String::new()))
}

// TODO: unwrap <=> closure
fn judge(dir: tempdir::TempDir, prob: &Problem) -> Vec<Case> {
    let exe_path = dir.path().join("code");
    log::info!("exe_path: {:?}", exe_path);
    //sleep(60u64, 0u32);
    prob.cases
        .iter()
        .enumerate()
        .map(|(id, case)| {
            let in_file = fs::File::open(&case.input_file).unwrap();
            let out_path = dir.path().join("code.out");
            let out_file = fs::File::create(&out_path).unwrap();
            // Run and estimate time
            let now = time::Instant::now();
            let mut child = std::process::Command::new(exe_path.to_str().unwrap())
                .stdin(in_file)
                .stdout(out_file)
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let duration = time::Duration::from_micros(case.time_limit as u64 + 5e5 as u64);
            let ret_code = match child.wait_timeout(duration).unwrap() {
                Some(status) => status.code(),
                None => {
                    child.kill().unwrap();
                    None
                }
            };
            // Find out result
            let case_res = match ret_code {
                None => CaseResult::TimeLimitExceeded,
                Some(x) if x > 0 => CaseResult::RuntimeError,
                Some(0) => {
                    let ans = fs::read_to_string(&case.answer_file);
                    let status = match prob.r#type {
                        ProblemType::Standard => Command::new("diff")
                            .args(["-w", &case.answer_file, out_path.to_str().unwrap()])
                            .status()
                            .expect("diff error"),
                        ProblemType::Strict => Command::new("diff")
                            .args(["-w", &case.answer_file, out_path.to_str().unwrap()])
                            .status()
                            .expect("diff error"),
                        _ => unreachable!(),
                    };
                    if status.code().unwrap() == 0 {
                        CaseResult::Accepted
                    } else {
                        CaseResult::WrongAnswer
                    }
                }
                _ => unreachable!(),
            };
            let time = now.elapsed().as_micros();
            Case {
                id: id as i32,
                result: case_res,
                time: time as u64,
                memory: 0,
                info: String::new(),
            }
        })
        .collect()
}

#[post("/jobs")]
async fn post_jobs(body: web::Json<PostJob>, conf: web::Data<Conf>) -> Result<impl Responder> {
    let job = body.into_inner();
    check_contest(&conf, &job)?;
    check_prob_id(&conf, job.problem_id)?;
    check_language(&conf, &job)?;
    // Compile
    let dir = tempdir::TempDir::new("oj")?;
    let file_path = dir.path().join("code.txt");
    fs::write(&file_path, &job.source_code)?;
    let lang = conf
        .languages
        .iter()
        .find(|x| x.name == job.language.as_str());
    match lang {
        None => return err::actix_err(err::ErrorKind::ErrInvalidArgument, String::new()),
        Some(lang) => {
            let cmd = lang.command.clone();
            let cmd = cmd
                .into_iter()
                .map(|x| match x.as_str() {
                    "%INPUT%" => file_path.to_str().unwrap().to_string(),
                    "%OUTPUT%" => dir.path().join("code").to_str().unwrap().to_string(),
                    _ => x,
                })
                .collect::<Vec<String>>();
            log::info!("cmd: {:?}", cmd);
            let status = Command::new(&cmd[0])
                .args(&cmd[1..])
                .status()?;
            log::info!("status: {:?},", status);
            if !status.success() {
                // TODO Compilation Error
                return err::actix_err(err::ErrorKind::ErrInternal, "Compilation Error".to_string());
            }
        }
    };
    // Run
    let prob = &conf.problems[job.problem_id as usize];
    let cases = judge(dir, prob);
    let mut job_res = PostJobRes::new(job);
    job_res.load_cases(cases, prob);

    Ok(web::Json(job_res))
}
