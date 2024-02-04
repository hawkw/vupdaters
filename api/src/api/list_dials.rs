use super::{ApiError, Backlight, DialId, Response};
use http::{uri, Method};

pub type ListDialResponse = super::Response<Vec<Dial>>;

pub const PATH: &str = "/api/v0/dial/list";

pub fn request(uri: uri::Builder, key: &str) -> Result<http::Request<()>, http::Error> {
    let uri = uri.path_and_query(format!("{PATH}?key={key}")).build()?;
    http::Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(())
}

#[cfg(feature = "client")]
impl crate::client::Client {
    #[tracing::instrument(level = tracing::Level::DEBUG, skip(self))]
    pub async fn list_dials(&self) -> Result<Vec<Dial>, ApiError> {
        let url = self.cfg.base_url.join(PATH).expect("invalid base URL!");
        let response = self
            .client
            .get(url)
            .query(&[("key", &*self.cfg.key)])
            .send()
            .await?
            .error_for_status()?;

        tracing::debug!(status = ?response.status(), "list_dials response");
        if !response.status().is_success() {
            let status = response.status();
            if let Ok(body) = response.json::<Response<()>>().await {
                return Err(ApiError::ServerHttp {
                    status,
                    message: body.message,
                });
            } else {
                return Err(ApiError::ServerHttp {
                    status,
                    message: "<no message>".to_string(),
                });
            }
        }

        let json = response.json::<ListDialResponse>().await?;
        if json.status != super::Status::Ok {
            return Err(ApiError::Server(json.message));
        }
        Ok(json.data)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Dial {
    // TODO(eliza): make this a newtype..
    pub uid: DialId,
    pub dial_name: String,
    pub value: usize,
    pub backlight: Backlight,
    pub image_file: String,
}
