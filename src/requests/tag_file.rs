use std::borrow::Cow;

use super::{Body, DeserializeError, Parse, Request};

macro_rules! newtype {
    ($name:ident) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
        pub struct $name(u64);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl $name {
            pub fn into_inner(self) -> u64 {
                self.0
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }
    };
}

newtype!(TagId);
newtype!(FileId);

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
