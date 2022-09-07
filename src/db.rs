use crate::{
    conf::{Conf, Problem},
    err,
    err::raise_err,
    judge::{judge, CaseRes, CaseResult, PostJob, State},
    utils::apmax,
};
use actix_web::{get, post, put, web, Responder, Result};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
// use chrono::DateTime;

lazy_static! {
    // job, user: indexed
    // problem_id: arbitrary
    // MUST ACQUIRE JOB_SET BEFORE USER_VEC BEFORE CONTESTS
    static ref JOB_SET: Arc<Mutex<BTreeSet<PostJobRes>>> = Arc::new(Mutex::new(BTreeSet::new()));
    static ref USER_VEC: Arc<Mutex<Vec<User>>> = Arc::new(Mutex::new(Vec::new()));
    static ref CONTESTS: Arc<Mutex<Vec<Contest>>> = Arc::new(Mutex::new(Vec::new()));
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
    pub fn new_with_cases(job: PostJob, prob: &Problem) -> Self {
        let mut res = Self::new(job);
        res.cases = (0..=prob.cases.len())
            .map(|id| CaseRes {
                id: id as i32,
                result: CaseResult::Waiting,
                ..Default::default()
            })
            .collect();
        res
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

#[put("/jobs/{job_id}")] // Rejudge after done?
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
const fn nul_id() -> i32 {
    -1
}
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct User {
    #[serde(default = "nul_id")]
    id: i32,
    name: String,
}

pub fn init_user() {
    let mut users = USER_VEC.lock().unwrap();
    let mut contests = CONTESTS.lock().unwrap();
    contests[0].user_ids.push(0);
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
async fn post_user(user: web::Json<User>) -> Result<impl Responder> {
    let user = user.into_inner();
    let mut users = USER_VEC.lock().unwrap();
    let mut contests = CONTESTS.lock().unwrap();
    let len = users.len();
    if user.id == nul_id() {
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
        contests[0].user_ids.push(users.len() as i32 - 1);
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

// Contest Related
#[derive(Clone, Deserialize, Serialize, Default)]
pub struct Contest {
    #[serde(default = "nul_id")]
    id: i32,
    name: String,
    from: String,
    to: String,
    problem_ids: Vec<i32>,
    user_ids: Vec<i32>,
    submission_limit: i32,
}
#[derive(Clone, Default, Serialize)]
pub struct UserRank {
    user: User,
    rank: i32,
    #[serde(skip)]
    sub_cnt: i32,
    #[serde(skip)]
    sub_time: String,
    scores: Vec<f64>,
}

pub fn init_contest(conf: &Conf) {
    let mut contests = CONTESTS.lock().unwrap();
    let problem_ids: Vec<i32> = conf.problems.iter().map(|prob| prob.id).collect();
    contests.push(Contest {
        problem_ids: problem_ids,
        from: chrono::Utc::now().to_string(),
        to: String::from("99999999"),
        submission_limit: 99999999,
        ..Default::default()
    });
}

pub fn check_contest(job: &PostJob) -> Result<impl Responder> {
    use err::ErrorKind::*;
    let contests = CONTESTS.lock().unwrap();
    let jobs = JOB_SET.lock().unwrap();
    // NOT_FOUND
    let contest = match contests.get(job.contest_id as usize) {
        Some(contest) => contest,
        None => raise_err!(ErrNotFound, ""),
    };
    // INVALID_ARGUMENT
    if !contest.user_ids.iter().any(|id| job.user_id == *id) {
        raise_err!(ErrInvalidArgument, "user {} not found", job.user_id);
    }
    if !contest.problem_ids.iter().any(|id| job.problem_id == *id) {
        raise_err!(ErrInvalidArgument, "prob {} not found", job.problem_id);
    }
    let time = chrono::Utc::now().to_string(); // Or use created_time?
    if time < contest.from || time > contest.to {
        raise_err!(ErrInvalidArgument, "bad submission time");
    }
    // RATE_LIMIT
    let cnt = jobs
        .iter()
        .filter(|hist| {
            hist.submission.contest_id == contest.id && hist.submission.user_id == job.user_id
        })
        .count();
    if cnt as i32 >= contest.submission_limit {
        raise_err!(ErrRateLimit, "")
    }
    Ok("")
}

#[post("/contests")]
async fn post_contest(
    contest: web::Json<Contest>,
    conf: web::Data<Conf>,
) -> Result<impl Responder> {
    let users = USER_VEC.lock().unwrap();
    let mut contests = CONTESTS.lock().unwrap();
    // Check contests
    let contest = contest.into_inner();
    let invld_prob = contest
        .problem_ids
        .iter()
        .any(|id| !conf.problems.iter().any(|prob| prob.id == *id));
    let invld_user = contest.user_ids.iter().any(|id| users.len() as i32 <= *id);
    log::info!("invld_prob: {}, invld_user: {}", invld_prob, invld_user);
    if invld_prob || invld_user || contest.id == 0 {
        // TODO check contest 0 behavior
        raise_err!(err::ErrorKind::ErrNotFound, "");
    }

    if contest.id == nul_id() {
        let len = contests.len();
        let contest = Contest {
            id: len as i32,
            ..contest
        };
        contests.push(contest.clone());
        Ok(web::Json(contest))
    } else {
        if let Some(entry) = contests.get_mut(contest.id as usize) {
            *entry = contest.clone();
            Ok(web::Json(contest))
        } else {
            raise_err!(err::ErrorKind::ErrNotFound, "");
        }
    }
}

#[get("/contests")]
async fn get_contests() -> Result<impl Responder> {
    let mut contests: Vec<_> = CONTESTS.lock().unwrap().clone();
    contests.drain(0..1);
    Ok(web::Json(contests))
}

#[get("/contests/{id}")]
async fn get_contest(id: web::Path<i32>) -> Result<impl Responder> {
    let contests = CONTESTS.lock().unwrap();
    let id = id.into_inner();
    match contests.get(id as usize) {
        Some(contest) => Ok(web::Json(contest.clone())),
        None => raise_err!(err::ErrorKind::ErrNotFound, "Contest {} not found.", id),
    }
}

// TODO: switch to O(nlogn) version?
#[derive(Deserialize)]
struct RankRule {
    scoring_rule: Option<String>,
    tie_breaker: Option<String>,
}
#[get("/contests/{contest_id}/ranklist")]
async fn get_ranklist(
    contest_id: web::Path<i32>,
    rule: web::Query<RankRule>,
) -> Result<impl Responder> {
    let jobs = JOB_SET.lock().unwrap();
    let users = USER_VEC.lock().unwrap();
    let contests = CONTESTS.lock().unwrap();

    let id = contest_id.into_inner();
    let contest = match contests.get(id as usize) {
        Some(contest) => contest,
        None => raise_err!(err::ErrorKind::ErrNotFound, "Contest {} not found.", id),
    };
    let mut user_ids: Vec<i32> = match id {
        0 => (0..users.len() as i32).collect(),
        _ => contest.user_ids.clone(),
    };
    user_ids.sort();
    let mut res: Vec<UserRank> = user_ids
        .iter()
        .map(|&user_id| {
            let mut score_map: HashMap<i32, f64> = HashMap::new();
            let (mut sub_cnt, mut sub_time) = (0, String::new());
            for job in jobs.iter() {
                let sub = &job.submission;
                if (sub.contest_id == id || id == 0) && sub.user_id == user_id {
                    let mut updated = true;
                    score_map
                        .entry(sub.problem_id)
                        .and_modify(|s| match rule.scoring_rule.as_deref() {
                            Some("latest") | None => *s = job.score,
                            Some("highest") => {
                                if *s < job.score {
                                    *s = job.score;
                                } else {
                                    updated = false;
                                }
                            }
                            _ => unreachable!(),
                        })
                        .or_insert(job.score);
                    sub_cnt += 1;
                    if updated {
                        apmax(&mut sub_time, job.created_time.clone());
                    }
                }
            }
            let scores: Vec<f64> = contest
                .problem_ids
                .iter()
                .map(|&id| score_map.entry(id).or_default().clone())
                .collect();
            if &sub_time == "" {
                sub_time = String::from("9");
            }
            UserRank {
                user: users.get(user_id as usize).unwrap().clone(),
                rank: 0,
                scores,
                sub_cnt,
                sub_time,
            }
        })
        .collect();
    let rank_cmp = |a: &UserRank, b: &UserRank, tie_breaker: &Option<String>| {
        let res = (b.scores.iter().sum::<f64>())
            .partial_cmp(&a.scores.iter().sum::<f64>())
            .unwrap();
        if std::cmp::Ordering::Equal == res {
            match tie_breaker.as_deref() {
                None => res,
                Some("submission_time") => a.sub_time.cmp(&b.sub_time),
                Some("submission_count") => a.sub_cnt.cmp(&b.sub_cnt),
                Some("user_id") => a.user.id.cmp(&b.user.id),
                Some(_) => unreachable!(),
            }
        } else {
            res
        }
    };
    res.sort_by(|a, b| rank_cmp(a, b, &rule.tie_breaker));

    res[0].rank = 1;
    log::info!("{}: {}", res[0].user.id, res[0].sub_time);
    for i in 1..res.len() {
        // take for granted that user_id is increasing
        let diff = rank_cmp(&res[i], &res[i - 1], &rule.tie_breaker) != std::cmp::Ordering::Equal;
        log::info!("{}: {}", res[i].user.id, res[i].sub_time);
        res[i].rank = match diff {
            true => i as i32 + 1,
            false => res[i - 1].rank,
        };
    }
    Ok(web::Json(res))
}
