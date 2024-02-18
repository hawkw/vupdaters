use serde::{de, Deserialize, Serialize};
use serde_with::{serde_as, DeserializeFromStr, DisplayFromStr, SerializeDisplay};
use std::{fmt, str::FromStr, sync::Arc, time::Duration};
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
    pub value: Percent,
    pub rgbw: [Percent; 4],
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

#[serde_as]
#[derive(Debug, Copy, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Easing {
    pub backlight_step: Percent,

    #[serde(alias = "backlight_period_ms", deserialize_with = "deserialize_millis")]
    pub backlight_period: Duration,

    pub dial_step: Percent,

    #[serde(alias = "dial_period_ms", deserialize_with = "deserialize_millis")]
    pub dial_period: Duration,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Serialize)]
pub struct Percent(u8);

#[derive(Debug, Error, miette::Diagnostic)]
#[error("invalid percent {0}")]
#[help = "percents must be in the range 0-100"]
#[diagnostic(code(vu_api::errors::backlight_error))]
pub struct PercentError(u8);

#[derive(Debug, Copy, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Backlight {
    pub red: Percent,
    pub green: Percent,
    pub blue: Percent,
}

#[derive(Debug, Error, miette::Diagnostic)]
pub enum PercentParseError {
    #[error(transparent)]
    InvalidPercent(
        #[from]
        #[diagnostic(transparent)]
        PercentError,
    ),
    #[error("not a u8: {0}")]
    NotAU8(#[from] std::num::ParseIntError),
}

#[derive(Debug, Error, miette::Diagnostic)]
#[error("invalid {} value: {}", .field, .value.0)]
#[help = "red, green, and blue Percents must be in the range 0-100"]
#[diagnostic(code(vu_api::errors::backlight_error))]
pub struct BacklightError {
    #[source]
    #[diagnostic_source]
    value: PercentError,
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
            red: Percent::new(red).map_err(mkerr("red"))?,
            green: Percent::new(green).map_err(mkerr("green"))?,
            blue: Percent::new(blue).map_err(mkerr("blue"))?,
        })
    }
}

// === impl Percent ===

impl Percent {
    pub fn new(value: u8) -> Result<Self, PercentError> {
        if value > 100 {
            Err(PercentError(value))
        } else {
            Ok(Self(value))
        }
    }
}

fn deserialize_millis<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: de::Deserializer<'de>,
{
    let millis = deserializer.deserialize_any(IntsAreSometimesStrings)?;
    Ok(Duration::from_millis(millis))
}

impl<'de> Deserialize<'de> for Percent {
    fn deserialize<D>(deserializer: D) -> Result<Percent, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let int = deserializer.deserialize_any(IntsAreSometimesStrings)?;
        u8::try_from(int)
            .map_err(serde::de::Error::custom)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

impl From<Percent> for u8 {
    fn from(Percent(value): Percent) -> Self {
        value
    }
}

impl TryFrom<u8> for Percent {
    type Error = PercentError;

    fn try_from(value: u8) -> Result<Percent, Self::Error> {
        Percent::new(value)
    }
}

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl fmt::Debug for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)?;
        f.write_str("%")
    }
}

impl FromStr for Percent {
    type Err = PercentParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim().trim_end_matches('%').parse()?;
        Ok(Percent::new(value)?)
    }
}

/// Workaround for a Really Annoying bug in VU-Server where integers in JSON
/// responses are sometimes sent as strings, and sometimes sent as actual JSON
/// integers, with (apparently) no rhyme or reason.
struct IntsAreSometimesStrings;

impl<'de> de::Visitor<'de> for IntsAreSometimesStrings {
    type Value = u64;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "a positive integer (or the string representation thereof)"
        )
    }

    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v as u64)
    }

    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v as u64)
    }

    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v as u64)
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v)
    }

    fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_i64(v as i64)
    }

    fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_i64(v as i64)
    }

    fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_i64(v as i64)
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if v < 0 {
            return Err(de::Error::invalid_value(de::Unexpected::Signed(v), &self));
        }

        Ok(v as u64)
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        s.parse::<u64>()
            .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(s), &self))
    }
}
