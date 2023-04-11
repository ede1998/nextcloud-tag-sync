use std::borrow::Cow;

use askama::Template;
use snafu::prelude::*;
use tracing::{debug, trace};
use url::Url;

use crate::Config;

#[derive(Debug)]
pub struct Connection {
    host: Url,
    user: String,
    token: String,
    client: reqwest::Client,
}

impl Connection {
    pub fn from_config(config: &Config) -> Self {
        Self {
            client: reqwest::Client::default(),
            user: config.user.clone(),
            token: config.token.clone(),
            host: config.nextcloud_instance.clone(),
        }
    }

    pub async fn request<T>(&self, request: T) -> Result<T::Output, RequestError>
    where
        T: Request + Parse,
    {
        let url = request.url(&self.host, &self.user);
        let method = reqwest::Method::from_bytes(request.method().as_bytes()).unwrap();
        debug!("Starting request {method} {url}");
        let payload = self
            .client
            .request(method, url)
            .basic_auth(&self.user, Some(&self.token))
            .body(request.body().context(AskamaSnafu)?)
            .send()
            .await
            .context(ReqwestSnafu)?
            .text()
            .await
            .context(ReqwestSnafu)?;
        trace!("Received payload: {payload}");
        T::parse(&payload).context(DeserializeSnafu)
    }
}

pub trait Request: Template {
    fn method(&self) -> Cow<str>;
    fn endpoint(&self) -> Cow<str>;
    fn url(&self, host: &Url, _user: &str) -> Url {
        let url = host.join("remote.php/dav").expect("failed to create URL");
        url.join(&self.endpoint()).expect("failed to create URL")
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
