use crate::{
    conf::{Conf, Problem},
    err,
    err::raise_err,
    judge::{judge, CaseRes, CaseResult, PostJob, State},
};
use actix_web::{get, post, put, web, Responder, Result};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
// use chrono::DateTime;

lazy_static! {
    // TODO: big to small not small to big
    static ref JOB_SET: Arc<Mutex<BTreeSet<PostJobRes>>> = Arc::new(Mutex::new(BTreeSet::new()));
    static ref USER_VEC: Arc<Mutex<Vec<User>>> = Arc::new(Mutex::new(Vec::new()));
}

// Judge related
#[derive(Clone, Debug, Default, Serialize)]
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
            id: JOB_SET.lock().unwrap().len() as i32,
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
        // self.created_time == other.created_time
        self.id == other.id
    }
}
impl std::cmp::Eq for PostJobRes {}
impl std::cmp::PartialOrd for PostJobRes {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // TODO check whether bigger one becomes first
        // Some(self.created_time.cmp(&other.created_time))
        Some(self.id.cmp(&other.id))
    }
}
impl std::cmp::Ord for PostJobRes {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

// Query related
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

pub async fn upd_job(job_res: PostJobRes) -> Result<impl Responder> {
    let mut set = JOB_SET.lock().unwrap();
    set.replace(job_res);
    Ok(actix_web::HttpResponse::Ok())
}

#[get("/jobs/{job_id}")]
async fn get_job(job_id: web::Path<i32>) -> Result<impl Responder> {
    let set = JOB_SET.lock().unwrap();
    let ls: Vec<_> = set.iter().filter(|x| x.id == job_id.clone()).collect();
    assert!(ls.len() <= 1);
    if let Some(&job) = ls.get(0) {
        Ok(web::Json(job.clone()))
    } else {
        Err(err::Error::new(
            err::ErrorKind::ErrNotFound,
            format!("Job {} not found.", job_id),
        )
        .into())
    }
}

#[put("/jobs/{job_id}")]
async fn put_job(job_id: web::Path<i32>, conf: web::Data<Conf>) -> Result<impl Responder> {
    let set = JOB_SET.lock().unwrap();
    let tmp_res = PostJobRes {
        id: job_id.clone(),
        ..Default::default()
    };
    let mut job_res = if let Some(job_res) = set.get(&tmp_res) {
        job_res.clone()
    } else {
        raise_err!(err::ErrorKind::ErrNotFound, "Job {} not found.", job_id)
    };
    drop(set);
    if job_res.state != State::Finished {
        raise_err!(
            err::ErrorKind::ErrInvalidState,
            "Job {} not finished.",
            job_id
        )
    }
    job_res.updated_time = chrono::Utc::now().to_string();
    let case_res = judge(&job_res.submission, &conf)?;
    let prob = conf.check_prob_and_get(job_res.submission.problem_id)?;
    let job_res = job_res.merge(case_res, prob);
    upd_job(job_res.clone()).await?;
    Ok(web::Json(job_res))
}

#[get("/jobs")]
async fn get_jobs(info: web::Query<JobQuery>) -> Result<impl Responder> {
    let job_set = JOB_SET.lock().unwrap();
    let user_vec = USER_VEC.lock().unwrap();

    macro_rules! check_job {
        ($job: tt, $info: ident, $elm: ident) => {
            if let Some(elm) = &$info.$elm {
                if &$job.$elm != elm {
                    return false;
                }
            }
        };
    }
    let vec: Vec<PostJobRes> = job_set
        .iter()
        .filter(|job| {
            if let Some(user_name) = &info.user_name {
                let vec_user = user_vec.get(job.submission.user_id as usize).unwrap();
                if &vec_user.name != user_name {
                    return false;
                }
            }
            check_job!((job.submission), info, user_id);
            check_job!((job.submission), info, contest_id);
            check_job!((job.submission), info, problem_id);
            check_job!((job.submission), info, language);
            check_job!(job, info, state);
            check_job!(job, info, result);
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

// User related
fn nul_user_id() -> i32 {
    -1
}
#[derive(Clone, Deserialize, Serialize)]
pub struct User {
    #[serde(default = "nul_user_id")]
    id: i32,
    name: String,
}

pub fn init_user() {
    let mut users = USER_VEC.lock().unwrap();
    users.push(User {
        id: 0,
        name: "root".to_string(),
    });
}

pub fn check_user(id: i32) -> Result<()> {
    let users = USER_VEC.lock().unwrap();
    if users.len() > id as usize {
        Ok(())
    } else {
        raise_err!(err::ErrorKind::ErrNotFound, "")
    }
}

#[post("/users")]
//async fn post_user(user: web::Json<User>) -> Result<web::Json<User>> {
async fn post_user(user: web::Json<User>) -> Result<impl Responder> {
    let user = user.into_inner();
    let mut users = USER_VEC.lock().unwrap();
    let len = users.len();
    if user.id == -1 {
        if users.iter().any(|cur| cur.name == user.name) {
            raise_err!(
                err::ErrorKind::ErrInvalidArgument,
                "User name '{}' already exists",
                user.name
            )
        }
        let new_user = User {
            id: len as i32,
            name: user.name,
        };
        users.push(new_user.clone());
        Ok(web::Json(new_user))
    } else {
        if let Some(elm) = users.get_mut(user.id as usize) {
            *elm = user.clone();
            Ok(web::Json(user))
        } else {
            raise_err!(err::ErrorKind::ErrNotFound, "User {} not found.", user.id)
        }
    }
}

#[get("/users")]
async fn get_users() -> Result<impl Responder> {
    let users: Vec<User> = USER_VEC.lock().unwrap().clone();
    Ok(web::Json(users))
}
