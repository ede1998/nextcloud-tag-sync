use std::{path::PathBuf, sync::Arc};

use futures::FutureExt as _;
use snafu::prelude::*;
use tokio::task::JoinError;
use tracing::{debug, error};

use crate::{
    updater::LocalSnafu, Command, Config, FileSystem, IntoOk, Modification, PrefixMapping,
    TagAction, Tags,
};

use super::{FileSystemLoopError, LocalFsWalker};

pub struct LocalFs {
    config: Arc<Config>,
}

impl LocalFs {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

impl FileSystem for LocalFs {
    async fn create_repo(&mut self) -> Result<crate::Repository, crate::InitError> {
        let config = self.config.clone();
        tokio::task::spawn_blocking(move || LocalFsWalker::new(&config).build_repository())
            .map(|res| match res {
                Ok(Ok(o)) => Ok(o),
                Ok(Err(e)) => Err(e).context(FilesystemLoopSnafu),
                Err(e) => Err(e).context(JoinSnafu),
            })
            .await
            .context(LocalSnafu)
    }

    async fn update_tags<I: IntoIterator<Item = Command>>(&mut self, commands: I) {
        for cmd in commands {
            let path = cmd.path.clone();
            match run_command(
                cmd,
                &self.config.local_tag_property_name,
                &self.config.prefixes,
            ) {
                Ok(_) => {
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

    let (path, mut tags) = match get_tags_of_file(path, tag_property_name) {
        Ok(ok) => ok,
        Err(FileError::Untagged { path }) => (path, Tags::default()),
        Err(err) => return Err(err),
    };

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
    #[snafu(display("no tags on file {}", path.display()))]
    Untagged { path: PathBuf },
}

#[derive(Debug, Snafu)]
pub enum LocalError {
    Join { source: JoinError },
    FilesystemLoop { source: FileSystemLoopError },
}
