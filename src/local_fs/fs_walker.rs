use std::path::Path;

use snafu::prelude::*;
use tracing::{debug, error, warn};
use walkdir::WalkDir;

use crate::{Config, PrefixMapping, Repository};

use super::{get_tags_of_file, FileError};

pub struct LocalFsWalker<'a> {
    tag_property_name: &'a str,
    prefixes: &'a [PrefixMapping],
}

impl<'a> LocalFsWalker<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self {
            tag_property_name: &config.local_tag_property_name,
            prefixes: &config.prefixes,
        }
    }

    pub fn build_repository(&self) -> Result<Repository, FileSystemLoopError> {
        let mut repo = Repository::new(self.prefixes.into());
        for prefix in self.prefixes {
            let walker = WalkDir::new(prefix.local());
            for entry in walker {
                let path = match entry {
                    Ok(ok) => ok.into_path(),
                    Err(e) => {
                        error!("Could not access path: {e}");
                        if e.loop_ancestor().is_some() {
                            return Err(e).context(FileSystemLoopSnafu);
                        }
                        if e.path().map(Path::is_dir) == Some(true) {
                            warn!("ignoring all files in this subtree");
                        }
                        continue;
                    }
                };

                match get_tags_of_file(path, self.tag_property_name) {
                    Ok((path, tags)) => {
                        if !tags.is_empty() {
                            repo.insert_local(&path, tags);
                        }
                    }
                    Err(FileError::IsDirectory { .. }) => {}
                    Err(err @ FileError::Untagged { .. }) => {
                        debug!("skipping file: {err}");
                    }
                    Err(err) => error!("skipping file: {err}"),
                }
            }
        }

        Ok(repo)
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("File system loop detected: {source}"))]
pub struct FileSystemLoopError {
    pub source: walkdir::Error,
}
