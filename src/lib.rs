mod requests;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub use requests::{Connection, ListFilesWithTag, ListTags};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct SyncedPath<'prefix> {
    prefix: &'prefix PrefixMapping,
    path: PathBuf,
}

impl<'prefix> SyncedPath<'prefix> {
    fn local_file(&self) -> PathBuf {
        self.prefix.local.join(&self.path)
    }

    fn remote_file(&self) -> PathBuf {
        self.prefix.remote.join(&self.path)
    }

    fn from_local(local: &Path, repo: &'prefix Repository) -> Self {
        let (prefix, path) = repo.split_prefix(local, FileLocation::Local);
        SyncedPath {
            prefix,
            path: path.to_owned(),
        }
    }

    fn from_remote(remote: &Path, repo: &'prefix Repository) -> Self {
        let (prefix, path) = repo.split_prefix(remote, FileLocation::Remote);
        SyncedPath {
            prefix,
            path: path.to_owned(),
        }
    }
}

struct Tags(Vec<String>);

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct PrefixMapping {
    local: PathBuf,
    remote: PathBuf,
}

struct Repository<'prefix> {
    prefixes: &'prefix [PrefixMapping],
    files: HashMap<SyncedPath<'prefix>, Tags>,
}

impl<'prefix> Repository<'prefix> {
    fn split_prefix<'a>(
        &self,
        file: &'a Path,
        location: FileLocation,
    ) -> (&PrefixMapping, &'a Path) {
        self.prefixes
            .iter()
            .find_map(|prefix_map| {
                let prefix = match location {
                    FileLocation::Local => &prefix_map.local,
                    FileLocation::Remote => &prefix_map.remote,
                };
                file.strip_prefix(prefix)
                    .map(|suffix| (prefix_map, suffix))
                    .ok()
            })
            .unwrap_or_else(|| panic!("missing prefix for {}", file.display()))
    }

    pub fn insert(&mut self, file: SyncedPath) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileLocation {
    Local,
    Remote,
}
