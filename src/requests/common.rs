use std::{borrow::Cow, error::Error};

use askama::Template;
use snafu::prelude::*;

const USER: &str = "erik";
const TOKEN: &str = include_str!("../../helper-scripts/nextcloud-token.txt");

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
    pub async fn request<T>(&self, request: T) -> Result<T::Output, RequestError>
    where
        T: Request + Parse,
    {
        let method = reqwest::Method::from_bytes(request.method().as_bytes()).unwrap();
        let payload = self
            .client
            .request(method, request.url(&self.host, &self.user))
            .basic_auth(&self.user, Some(TOKEN))
            .body(request.body().context(AskamaSnafu)?)
            .send()
            .await
            .context(ReqwestSnafu)?
            .text()
            .await
            .context(ReqwestSnafu)?;
        T::parse(&payload).context(DeserializeSnafu)
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

pub trait Parse {
    type Output;
    fn parse(input: &str) -> Result<Self::Output, DeserializeError>;
}

pub fn empty_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match serde::Deserialize::deserialize(de)? {
        None | Some("") => Ok(None),
        Some(s) => s.parse().map(Some).map_err(serde::de::Error::custom),
    }
}

pub type DeserializeError = serde_path_to_error::Error<quick_xml::DeError>;

pub fn parse<'de, T: serde::Deserialize<'de>>(input: &'de str) -> Result<T, DeserializeError> {
    let deserializer = &mut quick_xml::de::Deserializer::from_str(input);
    serde_path_to_error::deserialize(deserializer)
}

#[derive(Debug, Snafu)]
pub enum RequestError {
    #[snafu(display("Failed to render request template: {source}"))]
    Askama { source: askama::Error },
    #[snafu(display("Request failed: {source}"))]
    Reqwest { source: reqwest::Error },
    #[snafu(display("Failed to deserialize response: {source}"))]
    Deserialize { source: DeserializeError },
}
