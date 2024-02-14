use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DeserializeFromStr, DisplayFromStr, SerializeDisplay};
use std::{fmt, str::FromStr, sync::Arc};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Hash, DeserializeFromStr, SerializeDisplay)]
pub struct Id(Arc<str>);

#[serde_as]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Status {
    #[serde_as(as = "DisplayFromStr")]
    pub index: usize,
    pub uid: Id,
    pub dial_name: String,
    pub value: Value,
    pub rgbw: [Value; 4],
    pub easing: Easing,
    pub fw_hash: String,
    pub fw_version: String,
    pub hw_version: String,
    pub protocol_version: String,
    pub backlight: Backlight,
    pub image_file: String,
    pub update_deadline: f64,
    pub value_changed: bool,
    pub backlight_changed: bool,
    pub image_changed: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Easing {
    pub dial_step: usize,
    pub dial_period: usize,
    pub backlight_step: usize,
    pub backlight_period: usize,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize)]
pub struct Value(u8);

#[derive(Debug, Error, miette::Diagnostic)]
#[error("invalid value {0}")]
#[help = "values must be in the range 0-100"]
#[diagnostic(code(vu_api::errors::backlight_error))]
pub struct ValueError(u8);

#[derive(Debug, Copy, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Backlight {
    pub red: Value,
    pub green: Value,
    pub blue: Value,
}

#[derive(Debug, Error, miette::Diagnostic)]
pub enum ValueParseError {
    #[error(transparent)]
    InvalidValue(
        #[from]
        #[diagnostic(transparent)]
        ValueError,
    ),
    #[error("not a u8: {0}")]
    NotAU8(#[from] std::num::ParseIntError),
}

#[derive(Debug, Error, miette::Diagnostic)]
#[error("invalid {} value: {}", .field, .value.0)]
#[help = "red, green, and blue values must be in the range 0-100"]
#[diagnostic(code(vu_api::errors::backlight_error))]
pub struct BacklightError {
    #[source]
    #[diagnostic_source]
    value: ValueError,
    field: &'static str,
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Id {
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
