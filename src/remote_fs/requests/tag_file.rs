use std::borrow::Cow;

use super::{Body, DeserializeError, Parse, Request};
use crate::remote_fs::{FileId, TagId};

pub struct TagFile {
    tag: TagId,
    file: FileId,
}

impl TagFile {
    pub fn new(tag: TagId, file: FileId) -> Self {
        Self { tag, file }
    }
}

impl Request for TagFile {
    fn method(&self) -> reqwest::Method {
        reqwest::Method::PUT
    }

    fn endpoint(&self) -> Cow<str> {
        format!("systemtags-relations/files/{}/{}", self.file, self.tag).into()
    }

    fn body(&self) -> Option<Body> {
        None
    }
}

impl Parse for TagFile {
    type Output = ();

    fn parse(_: &str) -> Result<Self::Output, DeserializeError> {
        // We don't expect anything here and if we get sth because
        // of an error (4XX/5XX), it's already handled prior.
        Ok(())
    }
}
