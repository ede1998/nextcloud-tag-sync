use std::borrow::Cow;

use askama::Template;
use reqwest::header::CONTENT_TYPE;
use snafu::{prelude::*, ResultExt};
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
        let method = request.method();
        let (body, mime_type) = match request.body() {
            Some(body) => (body.content.context(AskamaSnafu)?, body.mime_type),
            None => (String::new(), ""),
        };

        let _url1 = url.clone();
        let _method1 = method.clone();
        let _body1 = body.clone();

        debug!("Starting request {method} {url}");
        let payload = if true {
            let mut request_builder = self
                .client
                .request(method, url)
                .basic_auth(&self.user, Some(&self.token));

            if !body.is_empty() {
                request_builder = request_builder.header(CONTENT_TYPE, mime_type).body(body);
            }

            request_builder
                .send()
                .await
                .context(ReqwestSnafu)?
                .error_for_status()
                .context(ReqwestSnafu)?
                .text()
                .await
                .context(ReqwestSnafu)?
        } else {
            read_sample_data(method, url, &body)
        };
        trace!("Received payload: {payload}");

        if false {
            update_sample_data(_method1, _url1, &_body1, &payload).await;
        }

        T::parse(&payload).context(DeserializeSnafu)
    }
}

async fn update_sample_data(method: reqwest::Method, url: url::Url, body: &str, payload: &str) {
    static COUNT: tokio::sync::Mutex<usize> = tokio::sync::Mutex::const_new(0);
    let count = {
        let mut cnt = COUNT.lock().await;
        let x = *cnt;
        *cnt += 1;
        x
    };
    let mut f = std::fs::File::create(format!("request-{count}.txt")).unwrap();
    use std::io::Write;
    writeln!(f, "{method}").unwrap();
    writeln!(f, "{url}").unwrap();
    writeln!(f, "{body}").unwrap();
    write!(f, "{}", payload).unwrap();
}

fn read_sample_data(method: reqwest::Method, url: url::Url, body: &str) -> String {
    use std::io::Read;
    let start = format!("{method}\n{url}\n{body}\n");
    for entry in std::fs::read_dir("sample-data").unwrap() {
        let entry = entry.unwrap();
        let mut f = std::fs::File::open(entry.path()).unwrap();
        let mut content = String::new();
        f.read_to_string(&mut content).unwrap();
        if let Some(payload) = content.strip_prefix(&start) {
            return payload.to_owned();
        }
    }
    panic!("Failed to find file with {start}");
}

pub trait Request {
    fn method(&self) -> reqwest::Method;
    fn endpoint(&self) -> Cow<str>;
    fn url(&self, host: &Url, _user: &str) -> Url {
        let url = host.join("remote.php/dav/").expect("failed to create URL");
        url.join(&self.endpoint()).expect("failed to create URL")
    }

    fn body(&self) -> Option<Body>;
}

pub struct Body {
    pub content: askama::Result<String>,
    pub mime_type: &'static str,
}

impl<T: Template> From<&T> for Body {
    fn from(value: &T) -> Self {
        Body {
            content: value.render(),
            mime_type: T::MIME_TYPE,
        }
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

pub fn str_to_method(method: &str) -> reqwest::Method {
    method.try_into().expect("failed to create HTTP method")
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
