use crate::{
    conf::{Conf, Language, Problem, ProblemType},
    db::{upd_jobs, PostJobRes},
    err,
};
use actix_web::{post, web, Responder, Result};
use chrono;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    process::{Command, Stdio},
    time,
};
use wait_timeout::ChildExt;

#[derive(Clone, Serialize, Deserialize)]
pub struct PostJob {
    // TODO return bad request
    pub source_code: String,
    pub language: String,
    pub user_id: i32,
    pub contest_id: i32,
    pub problem_id: i32,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum State {
    Queueing,
    Running,
    Finished,
    Canceled,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[allow(dead_code)]
pub enum CaseResult {
    Accepted = 0,
    #[serde(rename = "Compilation Success")]
    CompilationSuccess = 1,
    Waiting = 2,
    #[serde(rename = "Wrong Answer")]
    WrongAnswer = 3,
    #[serde(rename = "Runtime Error")]
    RuntimeError = 4,
    #[serde(rename = "Time Limit Exceeded")]
    TimeLimitExceeded = 5,
    #[serde(rename = "Compilation Error")]
    CompilationError = 6,
    Running,
    #[serde(rename = "Memory Limit Exceeded")]
    MemoryLimitExceeded,
    #[serde(rename = "System Error")]
    SystemError,
    #[serde(rename = "SPJ Error")]
    SPJError,
    Skipped,
    #[default]
    UnInitialized,
}

#[derive(Clone, Default, Serialize)]
pub struct CaseRes {
    pub id: i32,
    pub result: CaseResult,
    pub time: u64,
    pub memory: i32,
    pub info: String,
}

fn check_contest(conf: &Conf, job: &PostJob) -> Result<(), err::Error> {
    // TODO
    Ok(())
}
fn check_prob_and_get(conf: &Conf, id: i32) -> Result<&Problem, err::Error> {
    for prob in conf.problems.iter() {
        if id == prob.id {
            log::info!("id: {}, prob_id: {}", id, prob.id);
            return Ok(&prob);
        }
    }
    Err(err::Error::new(err::ErrorKind::ErrNotFound, String::new()))
}
fn check_lang_and_get<'a>(conf: &'a Conf, job: &PostJob) -> Result<&'a Language, err::Error> {
    for lang in conf.languages.iter() {
        if lang.name == job.language {
            return Ok(lang);
        }
    }
    Err(err::Error::new(err::ErrorKind::ErrNotFound, String::new()))
}

// TODO: unwrap <=> closure
fn judge(dir: tempdir::TempDir, prob: &Problem) -> Vec<CaseRes> {
    let exe_path = dir.path().join("code");
    log::info!("exe_path: {:?}", exe_path);
    let mut res: Vec<CaseRes> = prob
        .cases
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
                    let status = match prob.r#type {
                        ProblemType::Standard => Command::new("diff")
                            .args(["-w", &case.answer_file, out_path.to_str().unwrap()])
                            .status()
                            .expect("diff error"),
                        ProblemType::Strict => Command::new("diff")
                            .args([&case.answer_file, out_path.to_str().unwrap()])
                            .status()
                            .expect("diff error"),
                        _ => todo!(),
                    };
                    if status.code().unwrap() == 0 {
                        CaseResult::Accepted
                    } else {
                        CaseResult::WrongAnswer
                    }
                }
                _ => unreachable!("ret_code"),
            };
            let time = now.elapsed().as_micros();
            CaseRes {
                id: (id + 1) as i32,
                result: case_res,
                time: time as u64,
                memory: 0,
                info: String::new(),
            }
        })
        .collect();
    // add Compilation result
    res.insert(
        0usize,
        CaseRes {
            result: CaseResult::CompilationSuccess,
            ..Default::default()
        },
    );
    res
}

#[post("/jobs")]
async fn post_jobs(body: web::Json<PostJob>, conf: web::Data<Conf>) -> Result<impl Responder> {
    let job = body.into_inner();
    check_contest(&conf, &job)?;
    let lang = check_lang_and_get(&conf, &job)?;
    let prob = check_prob_and_get(&conf, job.problem_id)?;
    // Compile
    let dir = tempdir::TempDir::new("oj")?;
    let file_path = dir.path().join(&lang.file_name);
    fs::write(&file_path, &job.source_code)?;
    let job_res = PostJobRes::new(job);

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
    let status = Command::new(&cmd[0]).args(&cmd[1..]).status()?;
    log::info!("status: {:?},", status);

    let cases = if !status.success() {
        // Compilation Error
        let mut cases = vec![CaseRes {
            result: CaseResult::CompilationError,
            ..Default::default()
        }];
        for id in 1..=prob.cases.len() {
            cases.push(CaseRes {
                id: id as i32,
                result: CaseResult::Waiting,
                ..Default::default()
            });
        }
        cases
    } else {
        // Compilation Success
        judge(dir, prob)
    };
    let job_res = job_res.merge(cases, prob);
    upd_jobs(job_res.clone()).await?;
    Ok(web::Json(job_res))
}
