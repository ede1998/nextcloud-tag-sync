use std::{borrow::Cow, path::Path};

use askama::Template;

use reqwest::header::HeaderMap;

use crate::FileId;

use super::{parse, str_to_method, Body, DeserializeError, Parse, Request};

#[derive(Template)]
#[template(path = "get_file_id.xml")]
pub struct GetFileId {
    path: String,
}

impl GetFileId {
    pub fn new(remote_path: &Path) -> Option<Self> {
        Some(Self {
            path: remote_path.to_str()?.to_owned(),
        })
    }
}

impl Request for GetFileId {
    fn method(&self) -> reqwest::Method {
        str_to_method("PROPFIND")
    }

    fn endpoint(&self) -> Cow<str> {
        unimplemented!("Handled by URL");
    }

    fn url(&self, host: &url::Url, user: &str) -> url::Url {
        let path = &self.path;
        host.join(&format!("remote.php/dav/files/{user}/{path}"))
            .expect("failed to create URL")
    }

    fn body(&self) -> Body {
        self.into()
    }
}

impl Parse for GetFileId {
    type Output = FileId;
    type Error = DeserializeError;

    fn parse(_: &HeaderMap, input: &str) -> Result<Self::Output, Self::Error> {
        let element: MultiStatus = parse(input)?;
        Ok(element.file_id)
    }
}

#[derive(Debug, serde_query::Deserialize)]
struct MultiStatus {
    #[query(".response.propstat.prop.fileid")]
    file_id: FileId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    fn deserialize_all_tags() {
        let input = include_str!("../../../test_data/file_id.xml");
        let file_id = GetFileId::parse(&HeaderMap::new(), input).unwrap();
        assert_eq!(file_id, FileId::from(52));
    }
}
