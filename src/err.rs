use actix_web::{
    error,
    http::{header::ContentType, StatusCode},
    HttpResponse,
};
use derive_more::{Display, Error};
use serde::Serialize;

#[derive(Display, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorKind {
    ErrInvalidArgument,
    ErrInvalidState,
    ErrNotFound,
    ErrRateLimit,
    ErrExternal,
    ErrInternal,
}

impl ErrorKind {
    fn get_code(&self) -> u8 {
        match *self {
            Self::ErrInvalidArgument => 1,
            Self::ErrInvalidState => 2,
            Self::ErrNotFound => 3,
            Self::ErrRateLimit => 4,
            Self::ErrExternal => 5,
            Self::ErrInternal => 6,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Error {
    code: u8,
    reason: ErrorKind,
    message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_json::to_string(&self).unwrap())
    }
}

impl Error {
    pub fn new(reason: ErrorKind, message: String) -> Self {
        Self {
            code: reason.get_code(),
            reason: reason,
            message: message,
        }
    }
}

impl error::ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        match self.code {
            1 | 2 | 4 => StatusCode::BAD_REQUEST,
            3 => StatusCode::NOT_FOUND,
            5 | 6 => StatusCode::INTERNAL_SERVER_ERROR,
            _ => unreachable!(),
        }
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::json())
            .body(self.to_string())
    }
}

macro_rules! raise_err {
    ($err: expr, $($args:tt)*) => {
        return Err(err::Error::new($err, format!($($args)*)).into())
    }
}
pub(crate) use raise_err;
