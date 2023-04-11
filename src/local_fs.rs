// recurse into all local prefixes
// find files with tags
// construct repositories

use std::path::PathBuf;

use snafu::prelude::*;
use tracing::{debug, error};
use walkdir::{DirEntry, WalkDir};

use crate::{IntoOk, PrefixMapping, Repository, Tags};

pub struct LocalFsWalker<'a> {
    tag_property_name: String,
    prefixes: &'a [PrefixMapping],
}

impl<'a> LocalFsWalker<'a> {
    pub fn new(prefixes: &'a [PrefixMapping]) -> Self {
        Self {
            tag_property_name: "user.xdg.tags".to_owned(),
            prefixes,
        }
    }

    pub fn set_tag_property_name(&mut self, tag_property_name: String) {
        self.tag_property_name = tag_property_name;
    }

    pub fn build_repository(&self) -> Result<Repository, FileSystemLoopError> {
        let mut repo = Repository::new(self.prefixes.into());
        for prefix in self.prefixes {
            let walker = WalkDir::new(prefix.local());
            for entry in walker {
                match self.get_tags_of_file(entry) {
                    Ok((path, tags)) => {
                        if !tags.is_empty() {
                            repo.insert_local(&path, tags);
                        }
                    }
                    Err(FileError::IsDirectory { .. }) => {}
                    Err(err) => match err {
                        FileError::WalkDir { source } if source.loop_ancestor().is_some() => {
                            return Err(source).context(FileSystemLoopSnafu);
                        }
                        err => error!("skipping file. {err}"),
                    },
                }
            }
        }

        Ok(repo)
    }

    fn get_tags_of_file(
        &self,
        entry: Result<DirEntry, walkdir::Error>,
    ) -> Result<(PathBuf, Tags), FileError> {
        let entry = entry.context(WalkDirSnafu)?;
        ensure!(
            entry.file_type().is_file(),
            IsDirectorySnafu {
                path: entry.into_path()
            }
        );
        let path = entry.into_path();

        debug!("reading tags of file {}", path.display());

        let tag = xattr::get(&path, &self.tag_property_name)
            .with_context(|_| XAttrSnafu { path: path.clone() })?
            .unwrap_or_default();
        let tag =
            String::from_utf8(tag).with_context(|_| TagsNotUtf8Snafu { path: path.clone() })?;

        #[allow(unstable_name_collisions)]
        Ok((path, tag.parse().into_ok()))
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("File system loop detected: {source}"))]
pub struct FileSystemLoopError {
    pub source: walkdir::Error,
}

#[derive(Debug, Snafu)]
enum FileError {
    #[snafu(display("Path {} is a directory", path.display()))]
    IsDirectory { path: PathBuf },
    #[snafu(display("Failed to access path: {source}"))]
    WalkDir { source: walkdir::Error },
    #[snafu(display("Failed to get tags for path {}: {source}", path.display()))]
    XAttr {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("Tags of file {} are not valid UTF-8: {source}", path.display()))]
    TagsNotUtf8 {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },
}
