use actix_web::{web, Result, Responder};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;
use crate::{
    conf::Problem,
    judge::{PostJob, State, CaseResult, CaseRes}
};
// use chrono::DateTime;

// Judge related
lazy_static! {
    static ref JOB_LIST: Arc<Mutex<Vec<PostJobRes>>> = Arc::new(Mutex::new(Vec::new()));
}

#[derive(Serialize)]
pub struct PostJobRes {
    id: i32,
    created_time: String, //chrono::DateTime<chrono::Utc>
    updated_time: String,
    submission: PostJob,
    state: State,
    result: CaseResult,
    score: f64,
    cases: Vec<CaseRes>,
}

impl PostJobRes {
    pub fn new(job: PostJob) -> Self {
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

    pub fn from(job: PostJob, cases: Vec<CaseRes>, prob: &Problem) -> Self {
        let mut ret = Self::new(job);
        let mut result = CaseResult::Accepted;
        let mut score = 0f64;
        for (case_res, case_cfg) in cases.iter().skip(1).zip(prob.cases.iter()) {
            if (result as i32) < (case_res.result as i32) {
                result = case_res.result.clone();
            }
            if case_res.result == CaseResult::Accepted {
                score += case_cfg.score;
            }
        }
        ret.state = State::Finished;
        log::info!("cases[0].result: {:?}", cases[0].result);
        if cases[0].result == CaseResult::CompilationError {
            result = CaseResult::CompilationError;
        }
        (ret.result, ret.score, ret.cases) = (result, score, cases);
        ret
    }
}

// query related
#[derive(Serialize, Deserialize)]
struct JobQuery{
    user_id: Option<i32>,
    user_name: Option<String>,
    contest_id: Option<i32>,
    problem_id: Option<i32>,
    language: Option<String>,
    // DateTime<chrono::Utc>
    from: Option<String>,
    to: Option<String>,
    state: Option<State>,
    result: Option<State>,
}

fn upd_jobs() -> Result<impl Responder> {
    Ok(actix_web::HttpResponse::Ok())
}

async fn get_jobs(info: web::Query<JobQuery>) -> Result<impl Responder> {
    // TODO! cannot use todo!()
    Ok(actix_web::HttpResponse::Ok())
}