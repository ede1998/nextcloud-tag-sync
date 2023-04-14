use std::borrow::Cow;

use askama::Template;

use super::{Body, DeserializeError, Parse, Request};

#[derive(Template)]
#[template(path = "create_tag.json", escape = "none")]
pub struct CreateTag {
    tag: String,
}

impl CreateTag {
    pub fn new(tag: String) -> Self {
        Self { tag }
    }
}

impl Request for CreateTag {
    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
    }

    fn endpoint(&self) -> Cow<str> {
        "systemtags".into()
    }

    fn body(&self) -> Option<Body> {
        Some(self.into())
    }
}

impl Parse for CreateTag {
    type Output = ();

    fn parse(_: &str) -> Result<Self::Output, DeserializeError> {
        // We don't expect anything here and if we get sth because
        // of an error (4XX/5XX), it's already handled prior.
        Ok(())
    }
}
