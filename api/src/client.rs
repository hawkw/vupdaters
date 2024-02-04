use http::uri::Authority;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Client {
    pub(crate) key: Arc<str>,
    pub(crate) uri: Arc<str>,
    pub(crate) client: reqwest::Client,
}

impl Client {
    pub fn new(key: String, uri: String) -> Self {
        Self {
            key: Arc::from(key),
            uri: Arc::from(uri),
            client: reqwest::Client::new(),
        }
    }
}
