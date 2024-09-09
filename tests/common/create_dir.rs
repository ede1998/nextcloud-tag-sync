use std::{borrow::Cow, convert::Infallible};

use nextcloud_tag_sync::{Parse, Request};
use reqwest::header::HeaderMap;

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
    type Output = ();
    type Error = Infallible;

    fn parse(_: &HeaderMap, _: &str) -> Result<Self::Output, Self::Error> {
        // We don't expect anything here and if we get sth because
        // of an error (4XX/5XX), it's already handled prior.
        Ok(())
    }
}
