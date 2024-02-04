use super::{ApiError, Backlight, DialId, Response};
use serde_with::{serde_as, DisplayFromStr};
pub type DialStatusResponse = Response<DialStatus>;

#[cfg(feature = "client")]
impl crate::client::Client {
    #[tracing::instrument(level = tracing::Level::DEBUG, skip(self))]
    pub async fn dial_status(&self, dial: &DialId) -> Result<DialStatus, ApiError> {
        use reqwest::Url;

        let url = Url::parse(&format!("{}api/v0/dial/{dial}/status", self.cfg.base_url))
            .expect("invalid base URL!");
        let response = self
            .client
            .get(url)
            .query(&[("key", &*self.cfg.key)])
            .send()
            .await?
            .error_for_status()?;
        tracing::debug!(status = %response.status(), "dial status response");

        let json = response.json::<DialStatusResponse>().await?;
        if json.status != super::Status::Ok {
            return Err(ApiError::Server(json.message));
        }
        Ok(json.data)
    }
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DialStatus {
    #[serde_as(as = "DisplayFromStr")]
    pub index: usize,
    pub uid: DialId,
    pub dial_name: String,
    pub value: usize,
    pub rgbw: [u8; 4],
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
