use std::{borrow::Cow, convert::Infallible};

use nextcloud_tag_sync::{Body, Parse, Request};
use reqwest::header::HeaderMap;

pub struct UploadFile {
    path: String,
    contents: Vec<u8>,
}

impl UploadFile {
    #[must_use]
    pub fn new(path: impl Into<String>, contents: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            contents,
        }
    }
}

impl Request for UploadFile {
    fn method(&self) -> reqwest::Method {
        reqwest::Method::PUT
    }

    fn endpoint(&self) -> Cow<str> {
        unimplemented!("Handled by URL");
    }

    fn url(&self, host: &url::Url, user: &str) -> url::Url {
        let path = &self.path;
        host.join(&format!("remote.php/dav/files/{user}/{path}"))
            .expect("failed to create URL")
    }

    fn body(&self) -> Body {
        Body::Raw(self.contents.clone())
    }
}

impl Parse for UploadFile {
    type Output = ();
    type Error = Infallible;

    fn parse(_: &HeaderMap, _: &str) -> Result<Self::Output, Self::Error> {
        // We don't expect anything here and if we get sth because
        // of an error (4XX/5XX), it's already handled prior.
        Ok(())
    }
}
