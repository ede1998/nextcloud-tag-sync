use std::{borrow::Cow, num::ParseIntError, str::Utf8Error};

use nextcloud_tag_sync::{Body, FileId, Parse, Request};
use reqwest::header::{HeaderMap, HeaderValue};
use snafu::{OptionExt, ResultExt, Snafu};

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
        (&self.path).into()
    }

    fn url(&self, host: &reqwest::Url, _user: &str) -> reqwest::Url {
        host.join(&self.endpoint()).expect("failed to create URL")
    }

    fn body(&self) -> Body {
        Body::Raw(self.contents.clone())
    }
}

impl Parse for UploadFile {
    type Output = FileId;
    type Error = ParseFileTagError;

    fn parse(h: &HeaderMap, _: &str) -> Result<Self::Output, Self::Error> {
        extract_file_id(h)
    }
}

pub fn extract_file_id(h: &HeaderMap) -> Result<FileId, ParseFileTagError> {
    let header_value = h.get("oc-fileid").context(MissingFileIdHeaderSnafu)?;
    let global_id = std::str::from_utf8(header_value.as_bytes())
        .with_context(|_| NonUtf8FileIdSnafu { header_value })?;

    let file_id = global_id
        .split_once(|c: char| !c.is_numeric())
        .map_or(global_id, |g| g.0);

    file_id.parse().with_context(|_| FileIdParseSnafu {
        header_value,
        file_id,
    })
}

#[derive(Debug, Snafu)]
pub enum ParseFileTagError {
    #[snafu(display("Header 'oc-fileid' was missing from response",))]
    MissingFileIdHeader,
    #[snafu(display("Failed to parse file id {file_id} because of non-numeric symbols: {source}"))]
    FileIdParseError {
        header_value: HeaderValue,
        file_id: String,
        source: ParseIntError,
    },
    #[snafu(display("Failed to parse header {header_value:?} because of invalid UTF-8: {source}"))]
    NonUtf8FileId {
        header_value: HeaderValue,
        source: Utf8Error,
    },
}
