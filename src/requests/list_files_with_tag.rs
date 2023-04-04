use std::borrow::Cow;

use askama::Template;

use super::{parse, DeserializeError, Parse, Request};

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

impl Parse for ListFilesWithTag {
    type Output = Vec<String>;

    fn parse(input: &str) -> Result<Self::Output, DeserializeError> {
        #[derive(Debug, serde_query::Deserialize)]
        struct MultiStatus {
            #[query(".response.[].href")]
            files: Vec<String>,
        }
        let element: MultiStatus = parse(input)?;

        Ok(element.files)
    }
}
