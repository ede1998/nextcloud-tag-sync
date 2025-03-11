use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use futures::FutureExt as _;
use snafu::prelude::*;
use tokio::task::JoinError;
use tracing::{debug, error};

use crate::{
    Command, Config, FileSystem, IntoOk, Modification, PrefixMapping, TagAction, Tags,
    updater::LocalSnafu,
};

use super::LocalFsWalker;

#[derive(Debug)]
pub struct LocalFs {
    config: Arc<Config>,
}

impl LocalFs {
    #[must_use]
    pub const fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

impl FileSystem for LocalFs {
    async fn create_repo(&mut self) -> Result<crate::Repository, crate::InitError> {
        let config = self.config.clone();
        tokio::task::spawn_blocking(move || LocalFsWalker::new(&config).build_repository())
            .map(|res| match res {
                Ok(o) => Ok(o),
                Err(e) => Err(e).context(JoinSnafu),
            })
            .await
            .context(LocalSnafu)
    }

    async fn update_tags<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = Command> + Send,
    {
        for cmd in commands {
            let path = cmd.path.clone();
            match run_command(
                cmd,
                &self.config.local_tag_property_name,
                &self.config.prefixes,
            ) {
                Ok(()) => {
                    debug!("Successfully updated tags for file {path}");
                }
                Err(e) => {
                    error!("Failed to update tags for file {path}: {e}");
                }
            }
        }
    }
}

fn run_command(
    cmd: Command,
    tag_property_name: &str,
    prefixes: &[PrefixMapping],
) -> Result<(), FileError> {
    let path = cmd.path.local_file(prefixes);

    let mut tags = get_tags_of_file(&path, tag_property_name)?;

    for TagAction { tag, modification } in cmd.actions {
        match modification {
            Modification::Add => tags.insert_one(tag),
            Modification::Remove => tags.remove_one(&tag),
        }
    }

    xattr::set(&path, tag_property_name, tags.to_string().as_bytes())
        .with_context(|_| XAttrSnafu { path })?;

    Ok(())
}

/// Load all tags of the given local file using its extended attributes.
///
/// # Errors
///
/// This function will return an error if any of these is true:
/// - the file has no tags
/// - any tag is invalid
/// - the path is not a file
pub fn get_tags_of_file(path: &Path, tag_property_name: &str) -> Result<Tags, FileError> {
    ensure!(path.is_file(), IsDirectorySnafu { path });

    debug!("reading tags of file {}", path.display());

    let tag = xattr::get(path, tag_property_name)
        .with_context(|_| XAttrSnafu { path })?
        .unwrap_or_default();
    let tag = String::from_utf8(tag).with_context(|_| TagsNotUtf8Snafu { path })?;

    #[allow(unstable_name_collisions)]
    Ok(tag.parse().into_ok())
}

#[derive(Debug, Snafu)]
pub enum FileError {
    #[snafu(display("path {} is a directory", path.display()))]
    IsDirectory { path: PathBuf },
    #[snafu(display("could not get/set extended file attributes of {}: {source}", path.display()))]
    XAttr {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("tags of {} are not valid UTF-8: {source}", path.display()))]
    TagsNotUtf8 {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },
}

#[derive(Debug, Snafu)]
pub enum LocalError {
    Join { source: JoinError },
}
