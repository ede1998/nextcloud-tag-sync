use std::{borrow::Cow, convert::Infallible};

use reqwest::header::HeaderMap;

use super::{Body, Parse, Request};
use crate::remote_fs::{FileId, TagId};

pub struct UntagFile {
    tag: TagId,
    file: FileId,
}

impl UntagFile {
    #[must_use]
    pub const fn new(tag: TagId, file: FileId) -> Self {
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
    type Error = Infallible;

    fn parse(_: &HeaderMap, _: &str) -> Result<Self::Output, Self::Error> {
        // We don't expect anything here and if we get sth because
        // of an error (4XX/5XX), it's already handled prior.
        Ok(())
    }
}
