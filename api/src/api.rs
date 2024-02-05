use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{fmt, str::FromStr, sync::Arc};
use thiserror::Error;

pub mod dial_status;
pub mod list_dials;
pub mod set_dial;

/// A [response] from the VU API server.
///
/// [response]: https://docs.vudials.com/api_messaging/
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Response<T> {
    pub status: Status,
    pub message: String,
    pub data: T,
}

#[derive(Debug, Error, miette::Diagnostic)]
pub enum ApiError {
    #[error("failed to build request")]
    BuildRequest(#[from] http::Error),
    #[cfg(feature = "client")]
    #[error("client request failed")]
    Request(#[from] reqwest::Error),
    #[cfg(feature = "client")]
    #[error("server returned {}: {}", .status, .message)]
    ServerHttp {
        status: reqwest::StatusCode,
        message: String,
    },
    #[error("VU-Server API error: {}", .0)]
    Server(String),
    #[error("invalid backlight configuration: {0}")]
    InvalidBacklight(#[from] BacklightError),
    #[error("{0}")]
    InvalidValue(#[from] ValueError),
}

#[derive(Debug, Error, miette::Diagnostic)]
pub enum ValueParseError {
    #[error("{0}")]
    InvalidValue(#[from] ValueError),
    #[error("not a u8: {0}")]
    NotAU8(#[from] std::num::ParseIntError),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, DeserializeFromStr, SerializeDisplay)]
pub struct DialId(Arc<str>);

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Backlight {
    pub red: Value,
    pub green: Value,
    pub blue: Value,
}

#[derive(Debug, Error, miette::Diagnostic)]
#[error("invalid {} value: {}", .field, .value.0)]
#[help = "red, green, and blue values must be in the range 0-100"]
pub struct BacklightError {
    #[source]
    value: ValueError,
    field: &'static str,
}

/// The [status] of a response from the VU API server.
///
/// [status]: https://docs.vudials.com/api_messaging/#status
#[derive(Copy, Clone, Debug, DeserializeFromStr, SerializeDisplay, PartialEq, Eq, Hash)]
pub enum Status {
    Ok,
    Fail,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListDials {}

#[derive(Debug, Error)]
#[error("expected one of 'ok' or 'fail'")]
pub struct InvalidStatus(());

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize)]
pub struct Value(u8);

#[derive(Debug, Error, miette::Diagnostic)]
#[error("invalid value {0}")]
#[help = "values must be in the range 0-100"]
pub struct ValueError(u8);

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

impl fmt::Display for DialId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for DialId {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned().into()))
    }
}

impl Backlight {
    pub fn new(red: u8, green: u8, blue: u8) -> Result<Self, BacklightError> {
        let mkerr = |field: &'static str| move |value| BacklightError { value, field };
        Ok(Self {
            red: Value::new(red).map_err(mkerr("red"))?,
            green: Value::new(green).map_err(mkerr("green"))?,
            blue: Value::new(blue).map_err(mkerr("blue"))?,
        })
    }
}

// === impl Value ===

impl Value {
    pub fn new(value: u8) -> Result<Self, ValueError> {
        if value > 100 {
            Err(ValueError(value))
        } else {
            Ok(Self(value))
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Value::new(value).map_err(serde::de::Error::custom)
    }
}

impl From<Value> for u8 {
    fn from(Value(value): Value) -> Self {
        value
    }
}

impl TryFrom<u8> for Value {
    type Error = ValueError;

    fn try_from(value: u8) -> Result<Value, Self::Error> {
        Value::new(value)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Value {
    type Err = ValueParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim().parse()?;
        Ok(Value::new(value)?)
    }
}
