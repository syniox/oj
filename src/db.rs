use crate::{
    conf::Problem,
    judge::{CaseRes, CaseResult, PostJob, State},
};
use actix_web::{get, web, Responder, Result};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
// use chrono::DateTime;

// Judge related
lazy_static! {
    static ref JOB_SET: Arc<Mutex<BTreeSet<PostJobRes>>> = Arc::new(Mutex::new(BTreeSet::new()));
}

#[derive(Clone, Serialize)]
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

    pub fn merge(mut self, cases: Vec<CaseRes>, prob: &Problem) -> Self {
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
        self.state = State::Finished;
        log::info!("cases[0].result: {:?}", cases[0].result);
        if cases[0].result == CaseResult::CompilationError {
            result = CaseResult::CompilationError;
        }
        (self.result, self.score, self.cases) = (result, score, cases);
        self
    }
}

impl std::cmp::PartialEq for PostJobRes {
    fn eq(&self, other: &Self) -> bool {
        // TODO: is this enough?
        self.created_time == other.created_time
    }
}
impl std::cmp::Eq for PostJobRes {}
impl std::cmp::PartialOrd for PostJobRes {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // TODO check whether bigger one becomes first
        Some(self.created_time.cmp(&other.created_time))
    }
}
impl std::cmp::Ord for PostJobRes {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

// query related
#[derive(Serialize, Deserialize)]
struct JobQuery {
    user_id: Option<i32>,
    user_name: Option<String>,
    contest_id: Option<i32>,
    problem_id: Option<i32>,
    language: Option<String>,
    // DateTime<chrono::Utc>
    from: Option<String>,
    to: Option<String>,
    state: Option<State>,
    result: Option<CaseResult>,
}

pub async fn upd_jobs(job_res: PostJobRes) -> Result<impl Responder> {
    let mut set = JOB_SET.lock().unwrap();
    set.replace(job_res);
    Ok(actix_web::HttpResponse::Ok())
}

#[get("/jobs")]
async fn get_jobs(info: web::Query<JobQuery>) -> Result<impl Responder> {
    // TODO! cannot use todo!()
    let set = JOB_SET.lock().unwrap();

    macro_rules! check_submit {
        ($job: ident, $info: ident, $e: ident) => {
            if let Some(e) = &$info.$e {
                if &$job.submission.$e != e {
                    return false;
                }
            }
        };
    }

    let vec: Vec<PostJobRes> = set
        .iter()
        .filter(|job| {
            if let Some(_user_name) = &info.user_name {
                todo!();
            }
            check_submit!(job, info, user_id);
            check_submit!(job, info, contest_id);
            check_submit!(job, info, problem_id);
            check_submit!(job, info, language);
            if let Some(state) = &info.state {
                if &job.state != state {
                    return false;
                }
            }
            if let Some(result) = &info.result {
                if &job.result != result {
                    return false;
                }
            }
            if let Some(from) = &info.from {
                if from.cmp(&job.created_time) == std::cmp::Ordering::Greater {
                    return false;
                }
            }
            if let Some(to) = &info.to {
                if to.cmp(&job.created_time) == std::cmp::Ordering::Less {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();
    Ok(web::Json(vec))
}