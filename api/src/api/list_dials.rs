use super::{ApiError, Response};
use http::{uri, Method};

pub type ListDialResponse = super::Response<Vec<Dial>>;

pub fn request(uri: uri::Builder, key: &str) -> Result<http::Request<()>, http::Error> {
    let uri = uri
        .path_and_query(format!("/api/v0/dial/list?key={key}"))
        .build()?;
    http::Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(())
}

#[cfg(feature = "client")]
impl crate::client::Client {
    pub async fn list_dials(&self) -> Result<Vec<Dial>, ApiError> {
        let request = self
            .client
            .get(format!("{}/api/v0/dial/list", self.uri))
            .header("host", &*self.uri)
            .query(&[("key", &*self.key)])
            .send()
            .await?;
        let rsp = request.json::<ListDialResponse>().await?;
        if rsp.status != super::Status::Ok {
            return Err(ApiError::Server(rsp.message));
        }
        Ok(rsp.data)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Dial {
    // TODO(eliza): make this a newtype..
    pub uid: String,
    pub dial_name: String,
    pub value: usize,
    pub backlight: Backlight,
    pub image_file: String,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Backlight {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}
