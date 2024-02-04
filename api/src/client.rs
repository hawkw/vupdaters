use crate::api::ApiError;
pub use reqwest::ClientBuilder;
use reqwest::{header::HeaderValue, IntoUrl, Response, Url};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Client {
    pub(crate) cfg: Arc<Config>,
    pub(crate) client: reqwest::Client,
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
