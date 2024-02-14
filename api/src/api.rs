use crate::dial;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{fmt, str::FromStr};
use thiserror::Error;

/// A [response] from the VU API server.
///
/// [response]: https://docs.vudials.com/api_messaging/
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Response<T> {
    pub status: Status,
    pub message: String,
    pub data: T,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DialInfo {
    pub uid: dial::Id,
    pub dial_name: String,
    pub value: dial::Value,
    pub backlight: dial::Backlight,
    pub image_file: String,
}

#[derive(Debug, Error, miette::Diagnostic)]
pub enum Error {
    #[error("failed to build request: {0}")]
    #[diagnostic(code(vu_api::errors::client_error))]
    BuildRequest(#[from] http::Error),

    #[cfg(feature = "client")]
    #[error(transparent)]
    #[diagnostic(code(vu_api::errors::client_error))]
    BuildUrl(#[from] url::ParseError),

    #[cfg(feature = "client")]
    #[error(transparent)]
    #[diagnostic(code(vu_api::errors::client_error))]
    Request(#[from] reqwest::Error),

    #[cfg(feature = "client")]
    #[error("server returned {}: {}", .status, .message)]
    #[diagnostic(code(vu_api::errors::server_error))]
    ServerHttp {
        status: reqwest::StatusCode,
        message: String,
    },

    #[error("VU-Server API error: {}", .0)]
    #[diagnostic(code(vu_api::errors::server_error))]
    Server(String),

    #[error(transparent)]
    #[diagnostic(transparent)]
    InvalidBacklight(#[from] dial::BacklightError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    InvalidValue(#[from] dial::ValueError),
}

/// The [status] of a response from the VU API server.
///
/// [status]: https://docs.vudials.com/api_messaging/#status
#[derive(Copy, Clone, Debug, DeserializeFromStr, SerializeDisplay, PartialEq, Eq, Hash)]
pub enum Status {
    Ok,
    Fail,
}

#[derive(Debug, Error)]
#[error("expected one of 'ok' or 'fail'")]
pub struct InvalidStatus(());

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Ok => f.write_str("ok"),
            Status::Fail => f.write_str("fail"),
        }
    }
}

impl FromStr for Status {
    type Err = InvalidStatus;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            s if s.eq_ignore_ascii_case("ok") => Ok(Status::Ok),
            s if s.eq_ignore_ascii_case("fail") => Ok(Status::Fail),
            _ => Err(InvalidStatus(())),
        }
    }
}
