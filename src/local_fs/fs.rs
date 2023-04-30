use std::path::PathBuf;

use snafu::prelude::*;
use tracing::debug;

use crate::{Command, Config, IntoOk, Tags};

pub struct LocalFs {}

impl LocalFs {}

pub async fn execute<I>(commands: I, config: &Config)
where
    I: IntoIterator<Item = Command>,
    I::IntoIter: Clone,
{
}

pub(crate) fn get_tags_of_file(
    path: PathBuf,
    tag_property_name: &str,
) -> Result<(PathBuf, Tags), FileError> {
    ensure!(path.is_file(), IsDirectorySnafu { path });

    debug!("reading tags of file {}", path.display());

    let tag = xattr::get(&path, tag_property_name)
        .with_context(|_| XAttrSnafu { path: path.clone() })?
        .unwrap_or_default();
    let tag = String::from_utf8(tag).with_context(|_| TagsNotUtf8Snafu { path: path.clone() })?;

    ensure!(!tag.is_empty(), UntaggedSnafu { path });

    #[allow(unstable_name_collisions)]
    Ok((path, tag.parse().into_ok()))
}

#[derive(Debug, Snafu)]
pub(crate) enum FileError {
    #[snafu(display("path {} is a directory", path.display()))]
    IsDirectory { path: PathBuf },
    #[snafu(display("failed to get tags for path {}: {source}", path.display()))]
    XAttr {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("tags of file {} are not valid UTF-8: {source}", path.display()))]
    TagsNotUtf8 {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },
    #[snafu(display("no tags on file {}", path.display()))]
    Untagged { path: PathBuf },
}
