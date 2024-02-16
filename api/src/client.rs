use crate::{
    api,
    dial::{self, Id, Value},
};
use core::fmt;
pub use reqwest::ClientBuilder;
use reqwest::{header::HeaderValue, IntoUrl, Method, Url};
use std::sync::Arc;
use tracing::Level;

#[derive(Debug, Clone)]
#[must_use]
pub struct Client {
    pub(crate) cfg: Arc<Config>,
    pub(crate) client: reqwest::Client,
}

#[derive(Debug)]
#[must_use]
pub struct Dial {
    uid: Id,
    client: crate::Client,
    base_url: Url,
}

#[derive(Debug)]
pub(crate) struct Config {
    pub(crate) key: String,
    pub(crate) base_url: Url,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum NewClientError {
    #[error("invalid VU-Server base URL: {0}")]
    InvalidBaseUrl(#[source] reqwest::Error),
    #[error("failed to build reqwest client: {0}")]
    BuildClient(#[source] reqwest::Error),
}

impl Client {
    pub fn new(key: String, base_url: impl reqwest::IntoUrl) -> Result<Self, NewClientError> {
        static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

        let builder = reqwest::Client::builder().user_agent(HeaderValue::from_static(USER_AGENT));
        Self::from_builder(builder, key, base_url)
    }

    pub fn dial(&self, uid: impl Into<Id>) -> Result<Dial, url::ParseError> {
        let uid = uid.into();
        let base_url = self.cfg.base_url.join(&format!("api/v0/dial/{uid}/"))?;
        Ok(Dial {
            uid,
            client: self.clone(),
            base_url,
        })
    }

    #[tracing::instrument(
        level = Level::DEBUG,
        skip(self),
        err(Display, level = Level::DEBUG),
    )]
    pub async fn list_dials(&self) -> Result<Vec<(Dial, api::DialInfo)>, api::Error> {
        let url = self.cfg.base_url.join("/api/v0/dial/list")?;
        let response = self
            .client
            .get(url)
            .query(&[("key", &*self.cfg.key)])
            .send()
            .await?
            .error_for_status()?;

        let mut dials = response_json::<Vec<api::DialInfo>>(response).await?;
        dials
            .drain(..)
            .map(|dialinfo| {
                let dial = self.dial(dialinfo.uid.clone())?;
                Ok((dial, dialinfo))
            })
            .collect()
    }

    pub fn from_builder(
        builder: ClientBuilder,
        key: String,
        base_url: impl IntoUrl,
    ) -> Result<Self, NewClientError> {
        let client = builder.build().map_err(NewClientError::BuildClient)?;
        let base_url = base_url
            .into_url()
            .map_err(NewClientError::InvalidBaseUrl)?;
        Ok(Self {
            cfg: Arc::new(Config { key, base_url }),
            client,
        })
    }
}

impl Dial {
    fn build_request(
        &self,
        method: Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, api::Error> {
        let Client {
            ref cfg,
            ref client,
        } = self.client;

        // TODO(eliza): i hate that Reqwest takes owned, non-ref-counted URLs
        // and we can't seem to cache these...maybe switch to raw Hyper?
        let url = self.base_url.join(path)?;
        Ok(client.request(method, url).query(&[("key", &*cfg.key)]))
    }

    pub fn id(&self) -> &Id {
        &self.uid
    }

    #[tracing::instrument(
        level = Level::DEBUG,
        name = "Dial::status",
        skip(self),
        fields(uid = %self.uid),
        err(Display, level = Level::DEBUG),
    )]
    pub async fn status(&self) -> Result<dial::Status, api::Error> {
        let response = self.build_request(Method::GET, "status")?.send().await?;
        response_json(response).await
    }

    #[tracing::instrument(
        level = Level::DEBUG,
        name = "Dial::set_name",
        skip(self),
        fields(uid = %self.uid),
        err(Display, level = Level::DEBUG),
    )]
    pub async fn set_name(&self, name: &str) -> Result<(), api::Error> {
        let rsp = self
            .build_request(Method::GET, "name")?
            .query(&[("name", name)])
            .send()
            .await?;
        response_json(rsp).await
    }

    #[tracing::instrument(
        level = Level::DEBUG,
        name = "Dial::set",
        skip(self),
        fields(uid = %self.uid),
        err(Display, level = Level::DEBUG),
    )]
    pub async fn set(&self, value: Value) -> Result<(), api::Error> {
        let rsp = self
            .build_request(Method::GET, "set")?
            .query(&[("value", &value)])
            .send()
            .await?;
        response_json(rsp).await
    }

    #[tracing::instrument(
        level = Level::DEBUG,
        name = "Dial::set_backlight",
        skip(self),
        fields(uid = %self.uid),
        err(Display, level = Level::DEBUG),
    )]
    pub async fn set_backlight(
        &self,
        dial::Backlight { red, green, blue }: dial::Backlight,
    ) -> Result<(), api::Error> {
        let rsp = self
            .build_request(Method::GET, "backlight")?
            .query(&[
                ("red", &red.to_string()),
                ("green", &green.to_string()),
                ("blue", &blue.to_string()),
            ])
            .send()
            .await?;
        response_json(rsp).await
    }

    #[tracing::instrument(
        level = Level::DEBUG,
        name = "Dial::set_image",
        skip(self, part),
        fields(uid = %self.uid),
        err(Display, level = Level::DEBUG),
    )]
    pub async fn set_image(
        &self,
        filename: &str,
        part: reqwest::multipart::Part,
        force: bool,
    ) -> Result<(), api::Error> {
        let part = part.file_name(filename.to_string());
        let multipart = reqwest::multipart::Form::new().part("imgfile", part);
        let mut req = self
            .build_request(Method::POST, "image/set")?
            .query(&[("imgfile", filename)]);
        if force {
            req = req.query(&[("force", "true")])
        }
        let rsp = req.multipart(multipart).send().await?;
        response_json(rsp).await
    }

    #[tracing::instrument(
        level = Level::DEBUG,
        name = "Dial::reload_hw_info",
        skip(self),
        fields(uid = %self.uid),
        err(Display, level = Level::DEBUG),
    )]
    pub async fn reload_hw_info(&self) -> Result<dial::Status, api::Error> {
        let rsp = self.build_request(Method::GET, "reload")?.send().await?;
        response_json(rsp).await
    }
}

impl fmt::Display for Dial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.uid, f)
    }
}

async fn response_json<T: serde::de::DeserializeOwned>(
    rsp: reqwest::Response,
) -> Result<T, api::Error> {
    tracing::debug!(rsp.http_status = %rsp.status(), "received response");
    let rsp = rsp.error_for_status()?;
    let json = rsp.json::<api::Response<T>>().await?;
    if json.status != api::Status::Ok {
        return Err(api::Error::Server(json.message));
    }

    Ok(json.data)
}
