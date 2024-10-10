use std::borrow::Cow;

use nextcloud_tag_sync::{FileId, Parse, Request};
use reqwest::header::HeaderMap;

use super::upload_file::ParseFileTagError;

pub struct CreateDirectory {
    path: String,
}

impl CreateDirectory {
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl Request for CreateDirectory {
    fn method(&self) -> reqwest::Method {
        reqwest::Method::from_bytes(b"MKCOL").expect("HTTP method should be valid")
    }

    fn endpoint(&self) -> Cow<str> {
        (&self.path).into()
    }

    fn url(&self, host: &reqwest::Url, _user: &str) -> reqwest::Url {
        host.join(&self.endpoint()).expect("failed to create URL")
    }
}

impl Parse for CreateDirectory {
    type Output = FileId;
    type Error = ParseFileTagError;

    fn parse(h: &HeaderMap, _: &str) -> Result<Self::Output, Self::Error> {
        super::upload_file::extract_file_id(h)
    }
}
