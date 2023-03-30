use std::{borrow::Cow, error::Error};

use askama::Template;

use crate::deserializers::Parse;

const USER: &str = "erik";
const TOKEN: &str = include_str!("../helper-scripts/nextcloud-token.txt");

#[derive(Debug)]
pub struct Connection {
    host: String,
    user: String,
    client: reqwest::Client,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            client: reqwest::Client::default(),
            user: USER.to_owned(),
            host: "https://cloud.erik-hennig.me".to_owned(),
        }
    }
}

impl Connection {
    pub async fn request<T>(&self, request: T) -> Result<T::Output, Box<dyn Error>>
    where
        T: Request + Parse,
    {
        let method = reqwest::Method::from_bytes(request.method().as_bytes()).unwrap();
        let payload = self
            .client
            .request(method, request.url(&self.host, &self.user))
            .basic_auth(&self.user, Some(TOKEN))
            .body(request.body()?)
            .send()
            .await?
            .text()
            .await?;
        T::parse(&payload)
    }
}

pub trait Request: Template {
    fn method(&self) -> Cow<str>;
    fn endpoint(&self) -> Cow<str>;
    fn url(&self, host: &str, _user: &str) -> String {
        format!("{host}/remote.php/dav/{}", self.endpoint())
    }
    fn body(&self) -> askama::Result<String> {
        self.render()
    }
}

#[derive(Template)]
#[template(path = "load_tags.xml")]
pub struct ListTags;

impl Request for ListTags {
    fn method(&self) -> Cow<str> {
        "PROPFIND".into()
    }

    fn endpoint(&self) -> Cow<str> {
        "systemtags".into()
    }
}

#[derive(Template)]
#[template(path = "list_files_with_tag.xml")]
pub struct ListFilesWithTag {
    tag_id: u64,
}

impl ListFilesWithTag {
    pub fn new(tag_id: u64) -> Self {
        Self { tag_id }
    }
}

impl Request for ListFilesWithTag {
    fn method(&self) -> Cow<str> {
        "REPORT".into()
    }

    fn endpoint(&self) -> Cow<str> {
        "files".into()
    }

    fn url(&self, host: &str, user: &str) -> String {
        format!("{host}/remote.php/dav/{}/{user}", self.endpoint())
    }
}
