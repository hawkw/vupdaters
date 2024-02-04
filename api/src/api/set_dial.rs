#[cfg(feature = "client")]
use super::{ApiError, DialId, Response, Value};

#[cfg(feature = "client")]
impl crate::client::Client {
    #[tracing::instrument(level = tracing::Level::DEBUG, skip(self))]
    pub async fn set_name(&self, dial: &DialId, name: &str) -> Result<(), ApiError> {
        use reqwest::Url;

        let url = Url::parse(&format!("{}api/v0/dial/{dial}/name", self.cfg.base_url))
            .expect("invalid base URL!");
        let response = self
            .client
            .get(url)
            .query(&[("key", &*self.cfg.key), ("name", name)])
            .send()
            .await?
            .error_for_status()?;
        tracing::debug!(status = %response.status(), "set name response");

        let json = response.json::<Response<()>>().await?;
        if json.status != super::Status::Ok {
            return Err(ApiError::Server(json.message));
        }
        Ok(())
    }

    #[tracing::instrument(level = tracing::Level::DEBUG, skip(self))]
    pub async fn set_value(&self, dial: &DialId, value: Value) -> Result<(), ApiError> {
        use reqwest::Url;
        let value = value.0.to_string();
        let url = Url::parse(&format!("{}api/v0/dial/{dial}/set", self.cfg.base_url))
            .expect("invalid base URL!");
        let response = self
            .client
            .get(url)
            .query(&[("key", &*self.cfg.key), ("value", &value)])
            .send()
            .await?
            .error_for_status()?;
        tracing::debug!(status = %response.status(), "set value response");

        let json = response.json::<Response<()>>().await?;
        if json.status != super::Status::Ok {
            return Err(ApiError::Server(json.message));
        }
        Ok(())
    }

    pub async fn set_image(
        &self,
        dial: &DialId,
        filename: &str,
        part: reqwest::multipart::Part,
        force: bool,
    ) -> Result<(), ApiError> {
        use reqwest::Url;
        let url = Url::parse(&format!(
            "{}api/v0/dial/{dial}/image/set",
            self.cfg.base_url
        ))
        .expect("invalid base URL!");
        let part = part.file_name(filename.to_string());
        let multipart = reqwest::multipart::Form::new().part("imgfile", part);
        let mut req = self
            .client
            .post(url)
            .query(&[("key", &*self.cfg.key), ("imgfile", filename)]);
        if force {
            req = req.query(&[("force", "true")])
        }
        let response = req.multipart(multipart).send().await?.error_for_status()?;
        tracing::debug!(status = %response.status(), "set image response");

        let json = response.json::<Response<()>>().await?;
        if json.status != super::Status::Ok {
            return Err(ApiError::Server(json.message));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "client")]
    #[tokio::test]
    async fn test_post_form_file() {
        let url = "http://httpbin.org/";
        static MEM_IMG: &[u8] = include_bytes!("../../../vupdate/assets/mem.png");
        let client =
            crate::client::Client::new("fake_key".to_string(), url).expect("client should work");

        let multipart = reqwest::multipart::Form::new().part(
            "imgfile",
            reqwest::multipart::Part::bytes(MEM_IMG).file_name("mem_img.png"),
        );
        let req = client
            .client
            .post(url)
            .query(&[("key", "fakekey")])
            .multipart(multipart);
        println!("request: {req:#?}");
        //     .send()
        //     .await
        //     .expect("send should work")
        //     .error_for_status()
        //     .expect("should be OK");
        // tracing::debug!(status = %response.status(), "set image response");

        // println!("response: {:#?}", response.text().await.unwrap());
    }
}
