use std::borrow::Cow;

use super::{Body, DeserializeError, Parse, Request};
use crate::remote_fs::{FileId, TagId};

pub struct UntagFile {
    tag: TagId,
    file: FileId,
}

impl UntagFile {
    pub fn new(tag: TagId, file: FileId) -> Self {
        Self { tag, file }
    }
}

impl Request for UntagFile {
    fn method(&self) -> reqwest::Method {
        reqwest::Method::DELETE
    }

    fn endpoint(&self) -> Cow<str> {
        format!("systemtags-relations/files/{}/{}", self.file, self.tag).into()
    }

    fn body(&self) -> Option<Body> {
        None
    }
}

impl Parse for UntagFile {
    type Output = ();

    fn parse(_: &str) -> Result<Self::Output, DeserializeError> {
        // We don't expect anything here and if we get sth because
        // of an error (4XX/5XX), it's already handled prior.
        Ok(())
    }
}
