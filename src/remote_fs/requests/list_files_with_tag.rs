use std::borrow::Cow;

use askama::Template;
use url::Url;

use crate::TagId;

use super::{common::str_to_method, parse, Body, DeserializeError, Parse, Request};

#[derive(Template)]
#[template(path = "list_files_with_tag.xml")]
pub struct ListFilesWithTag {
    tag: TagId,
}

impl ListFilesWithTag {
    pub fn new(tag: TagId) -> Self {
        Self { tag }
    }
}

impl Request for ListFilesWithTag {
    fn method(&self) -> reqwest::Method {
        str_to_method("REPORT")
    }

    fn endpoint(&self) -> Cow<str> {
        "files".into()
    }

    fn url(&self, host: &Url, user: &str) -> Url {
        let suffix = format!("remote.php/dav/{}/{user}", self.endpoint());
        host.join(&suffix).expect("failed to create URL")
    }

    fn body(&self) -> Option<Body> {
        Some(self.into())
    }
}

impl Parse for ListFilesWithTag {
    type Output = Vec<String>;

    fn parse(input: &str) -> Result<Self::Output, DeserializeError> {
        #[derive(Debug, serde::Deserialize)]
        struct MultiStatus {
            #[serde(default)]
            response: Vec<Response>,
        }
        #[derive(Debug, serde::Deserialize)]
        struct Response {
            href: String,
        }

        let element: MultiStatus = parse(input)?;

        Ok(element.response.into_iter().map(|r| r.href).collect())
    }
}
