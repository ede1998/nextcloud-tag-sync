use std::{borrow::Cow, num::ParseIntError};

use askama::Template;
use reqwest::header::{HeaderMap, ToStrError, CONTENT_LOCATION};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::{Tag, TagId};

use super::{Body, Parse, Request};

#[derive(Template)]
#[template(path = "create_tag.json", escape = "none")]
pub struct CreateTag {
    tag: Tag,
}

impl CreateTag {
    pub fn new(tag: Tag) -> Self {
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
    type Output = TagId;
    type Error = CreateTagError;

    fn parse(headers: &HeaderMap, _: &str) -> Result<Self::Output, Self::Error> {
        // We don't expect any body here. If there was a body because
        // of an error (4XX/5XX), it's already handled prior.

        let location = headers.get(CONTENT_LOCATION).context(MissingHeaderSnafu)?;
        let location = location.to_str().context(NotUtf8Snafu)?;

        let id = location
            .rsplit_once('/')
            .with_context(|| MissingTagIdSnafu {
                location: location.to_owned(),
            })?
            .1;
        let id: u64 = id.parse().with_context(|_| InvalidTagIdSnafu {
            tag_id: id.to_owned(),
        })?;

        Ok(id.into())
    }
}

#[derive(Debug, Snafu)]
pub enum CreateTagError {
    #[snafu(display("header content-location is missing"))]
    MissingHeader,
    #[snafu(display("could not find tag id in location {location}"))]
    MissingTagId {
        #[snafu(implicit(false))]
        location: String,
    },
    #[snafu(display("failed to parse tag id {tag_id}: {source}"))]
    InvalidTagId {
        tag_id: String,
        source: ParseIntError,
    },
    #[snafu(display("header value is not valid UTF-8: {source}"))]
    NotUtf8 { source: ToStrError },
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;
    use reqwest::header::{CACHE_CONTROL, EXPIRES, PRAGMA};

    fn header_map() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_LOCATION,
            HeaderValue::from_static("/remote.php/dav/systemtags/742"),
        );
        headers.insert(
            EXPIRES,
            HeaderValue::from_static("Thu, 19 Nov 1981 08:52:00 GMT"),
        );
        headers.insert(
            CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate"),
        );
        headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
        headers
    }

    #[test]
    fn parse_tag_creation() {
        let headers = header_map();
        let tag_id = CreateTag::parse(&headers, "").unwrap();
        assert_eq!(tag_id.into_inner(), 742);
    }

    #[test]
    fn missing_header() {
        let mut headers = header_map();
        headers.remove(CONTENT_LOCATION);

        let err = CreateTag::parse(&headers, "").unwrap_err();
        assert!(matches!(err, CreateTagError::MissingHeader));
    }

    #[test]
    fn missing_tag_id() {
        let mut headers = header_map();
        headers.insert(
            CONTENT_LOCATION,
            HeaderValue::from_static("something unexpected"),
        );

        let err = CreateTag::parse(&headers, "").unwrap_err();
        assert!(matches!(err, CreateTagError::MissingTagId { .. }));
    }

    #[test]
    fn invalid_tag_id() {
        let mut headers = header_map();
        headers.insert(
            CONTENT_LOCATION,
            HeaderValue::from_static("/remote.php/dav/systemtags/NotANumber"),
        );

        let err = CreateTag::parse(&headers, "").unwrap_err();
        assert!(matches!(err, CreateTagError::InvalidTagId { .. }));
    }

    #[test]
    fn not_uf8() {
        let mut headers = header_map();
        headers.insert(CONTENT_LOCATION, HeaderValue::from_bytes(&[128]).unwrap());

        let err = CreateTag::parse(&headers, "").unwrap_err();
        assert!(matches!(err, CreateTagError::NotUtf8 { .. }));
    }
}
