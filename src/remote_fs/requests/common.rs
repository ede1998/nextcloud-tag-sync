use std::borrow::Cow;

use askama::Template;
use reqwest::header::{HeaderMap, CONTENT_TYPE};
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
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            client: reqwest::Client::default(),
            user: config.user.clone(),
            token: config.token.clone(),
            host: config.nextcloud_instance.clone(),
        }
    }

    pub async fn request<T>(&self, request: T) -> Result<T::Output, RequestError<T::Error>>
    where
        T: Request + Parse + Send,
    {
        let url = request.url(&self.host, &self.user);
        let method = request.method();

        debug!("Starting request {method} {url}");
        let (payload, headers) = if true {
            let mut request_builder = self
                .client
                .request(method, url)
                .basic_auth(&self.user, Some(&self.token));

            match request.body() {
                Body::Askama { content, mime_type } => {
                    let body = content.context(AskamaSnafu)?;
                    request_builder = request_builder.header(CONTENT_TYPE, mime_type).body(body);
                }
                Body::Empty => {}
                Body::Raw(data) => {
                    request_builder = request_builder.body(data);
                }
            }

            let response = request_builder
                .send()
                .await
                .context(ReqwestSnafu)?
                .error_for_status()
                .context(ReqwestSnafu)?;

            let headers = response.headers().clone();
            let body = response.text().await.context(ReqwestSnafu)?;

            (body, headers)
        } else {
            //read_sample_data(&method, &url, &body)
            todo!()
        };
        trace!("Received payload {payload} and headers {headers:?}");

        // update_sample_data(&method1, &url1, &body1, &payload).await;

        T::parse(&headers, &payload).context(DeserializeSnafu)
    }
}

#[allow(
    dead_code,
    reason = "Used to save sample data for testing by manually changing code to call this function"
)]
async fn update_sample_data(method: &reqwest::Method, url: &url::Url, body: &[u8], payload: &str) {
    use std::io::Write;

    static COUNT: tokio::sync::Mutex<usize> = tokio::sync::Mutex::const_new(0);
    let count = {
        let mut cnt = COUNT.lock().await;
        let x = *cnt;
        *cnt += 1;
        x
    };
    let mut f = std::fs::File::create(format!("request-{count}.txt")).unwrap();
    writeln!(f, "{method}").unwrap();
    writeln!(f, "{url}").unwrap();
    writeln!(f, "{}", String::from_utf8_lossy(body)).unwrap();
    write!(f, "{payload}").unwrap();
}

#[allow(
    dead_code,
    reason = "Used to read sample data from local file for testing by manual edit"
)]
fn read_sample_data(method: &reqwest::Method, url: &url::Url, body: &str) -> String {
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

    fn body(&self) -> Body {
        Body::default()
    }
}

#[derive(Debug, Default)]
pub enum Body {
    Askama {
        content: askama::Result<String>,
        mime_type: &'static str,
    },
    Raw(Vec<u8>),
    #[default]
    Empty,
}

impl<T: Template> From<&T> for Body {
    fn from(value: &T) -> Self {
        Self::Askama {
            content: value.render(),
            mime_type: T::MIME_TYPE,
        }
    }
}

pub trait Parse {
    type Output;
    type Error: snafu::Error + 'static;
    /// Parses the response body of the request to the nextcloud API.
    ///
    /// # Errors
    ///
    /// This function will return an error if parsing failed.
    fn parse(headers: &HeaderMap, body: &str) -> Result<Self::Output, Self::Error>;
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
pub enum RequestError<DeserializeError: std::fmt::Display + std::error::Error + 'static> {
    #[snafu(display("Failed to render request template: {source}"))]
    Askama { source: askama::Error },
    #[snafu(display("Request failed: {source}"))]
    Reqwest { source: reqwest::Error },
    #[snafu(display("Failed to deserialize response: {source}"))]
    Deserialize { source: DeserializeError },
}
