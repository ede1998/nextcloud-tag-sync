use std::borrow::Cow;

use askama::Template;
use reqwest::header::HeaderMap;
use url::Url;

use crate::{FileId, TagId};

use super::{Body, DeserializeError, Parse, Request, common::str_to_method, parse};

/// List all files with the given tag. Directories are ignored.
#[derive(Template)]
#[template(path = "list_files_with_tag.xml")]
pub struct ListFilesWithTag {
    tag: TagId,
}

impl ListFilesWithTag {
    #[must_use]
    pub const fn new(tag: TagId) -> Self {
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

    fn body(&self) -> Body {
        self.into()
    }
}

impl Parse for ListFilesWithTag {
    type Output = Vec<(FileId, String)>;
    type Error = DeserializeError;

    fn parse(_: &HeaderMap, input: &str) -> Result<Self::Output, Self::Error> {
        let element: MultiStatus = parse(input)?;

        Ok(element
            .response
            .into_iter()
            .filter(|r| r.resource_type.collection.is_none())
            .map(|r| (r.file_id, r.href))
            .collect())
    }
}

#[derive(Debug, serde::Deserialize)]
struct MultiStatus {
    #[serde(default)]
    response: Vec<Response>,
}

#[derive(Debug, serde_query::Deserialize)]
struct Response {
    #[query(".href")]
    href: String,
    #[query(".propstat.prop.fileid")]
    file_id: FileId,
    #[query(".propstat.prop.resourcetype")]
    resource_type: ResourceType,
}

#[derive(Debug, serde::Deserialize)]
struct ResourceType {
    collection: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_tagged_files() {
        let input = include_str!("../../../test_data/airplanes.xml");
        let tags = ListFilesWithTag::parse(&HeaderMap::new(), input).unwrap();

        assert_eq!(tags.len(), 105);
        assert!(tags.iter().any(|(id, name)| *id == FileId::from(58_988)
            && name == "/remote.php/dav/files/erik/Pictures/2021/2021-09-22T12-50-42.jpg"));
        assert!(tags.iter().any(|(id, name)| *id == FileId::from(1_220_518)
            && name == "/remote.php/dav/files/erik/Pictures/2022/2022-07-16T17-15-28.jpg"));
        assert!(tags.iter().any(|(id, name)| *id == FileId::from(34_934)
            && name == "/remote.php/dav/files/erik/Pictures/2010/2010-07-10T14-02-59.jpg"));
    }
}
