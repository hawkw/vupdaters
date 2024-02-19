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
    pub value: dial::Percent,
    pub backlight: dial::Backlight,
    pub image_file: String,
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
