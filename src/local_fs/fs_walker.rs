use std::path::Path;

use snafu::prelude::*;
use tracing::{debug, error, warn};
use walkdir::WalkDir;

use crate::{Config, PrefixMapping, Repository};

use super::{FileError, get_tags_of_file};

pub struct LocalFsWalker<'a> {
    tag_property_name: &'a str,
    prefixes: &'a [PrefixMapping],
}

impl<'a> LocalFsWalker<'a> {
    #[expect(
        clippy::missing_const_for_fn,
        reason = "false positive, adding const leads to compiler error"
    )]
    #[must_use]
    pub fn new(config: &'a Config) -> Self {
        Self {
            tag_property_name: &config.local_tag_property_name,
            prefixes: &config.prefixes,
        }
    }

    /// Builds a tag repository for the local file system.
    ///
    /// # Panics
    ///
    /// Panics if an unsynced file is encountered.
    pub fn build_repository(&self) -> Repository {
        let mut repo = Repository::new(self.prefixes.into());
        for prefix in self.prefixes {
            let walker = WalkDir::new(prefix.local());
            for entry in walker {
                let Some(path) = get_path(entry) else {
                    continue;
                };

                match get_tags_of_file(&path, self.tag_property_name) {
                    Ok(tags) => {
                        if tags.is_empty() {
                            debug!("skipping file: {}", path.display());
                        } else {
                            repo.insert_local(&path, tags)
                                .expect("Unsynced file encountered during repo build.");
                        }
                    }
                    Err(FileError::IsDirectory { .. }) => {}
                    Err(err) => error!("skipping file: {err}"),
                }
            }
        }

        repo
    }
}

fn get_path(entry: Result<walkdir::DirEntry, walkdir::Error>) -> Option<std::path::PathBuf> {
    match entry {
        Ok(ok) => Some(ok.into_path()),
        Err(e) => {
            if let Some(target) = e.loop_ancestor() {
                warn!(
                    "File system contains a symbolic link loop. Skipping loop from {} to {}.",
                    e.path().unwrap_or_else(|| Path::new("???")).display(),
                    target.display()
                );
            } else {
                error!("Could not access path: {e}");
                if e.path().map(Path::is_dir) == Some(true) {
                    warn!("Ignoring all files in this subtree.");
                }
            }
            None
        }
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("File system loop detected: {source}"))]
pub struct FileSystemLoopError {
    pub source: walkdir::Error,
}
