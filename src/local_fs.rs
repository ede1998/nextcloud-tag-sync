// recurse into all local prefixes
// find files with tags
// construct repositories

use std::{hint::unreachable_unchecked, path::PathBuf};

use snafu::{prelude::*, Whatever};
use tracing::error;
use walkdir::{DirEntry, WalkDir};

use crate::{PrefixMapping, Repository, Tags};

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

    pub fn build_repository(&self) -> Result<Repository, Whatever> {
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
                    Err(LocalFsError::IsDirectory { .. }) => {}
                    Err(err) => {
                        if let LocalFsError::WalkDir { ref source } = err {
                            ensure_whatever!(
                                source.loop_ancestor().is_none(),
                                "Found file system loop."
                            );
                        }
                        error!("Skipping file. {err}");
                    }
                }
            }
        }

        Ok(repo)
    }

    fn get_tags_of_file(
        &self,
        entry: Result<DirEntry, walkdir::Error>,
    ) -> Result<(PathBuf, Tags), LocalFsError> {
        let entry = entry.context(WalkDirSnafu)?;
        ensure!(
            entry.file_type().is_file(),
            IsDirectorySnafu {
                path: entry.into_path()
            }
        );
        let tag = xattr::get(entry.path(), &self.tag_property_name)
            .with_context(|_| XAttrSnafu {
                path: entry.path().to_owned(),
            })?
            .unwrap_or_default();
        let tag = String::from_utf8(tag).with_context(|_| TagsNotUtf8Snafu {
            path: entry.path().to_owned(),
        })?;

        #[allow(unstable_name_collisions)]
        Ok((entry.into_path(), tag.parse().into_ok()))
    }
}

#[derive(Debug, Snafu)]
enum LocalFsError {
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

trait IntoOk {
    fn into_ok(self) -> Self::T;
    type T;
}

impl<T> IntoOk for Result<T, std::convert::Infallible> {
    type T = T;
    fn into_ok(self) -> T {
        match self {
            Ok(o) => o,
            // safe because Infallible can never be instantiated
            Err(_) => unsafe { unreachable_unchecked() },
        }
    }
}
