mod map;
mod requests;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub use map::BidirectionalMap;
pub use requests::{Connection, ListFilesWithTag, ListTags};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
struct PrefixId(usize);

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct SyncedPath {
    prefix_id: PrefixId,
    path: String,
}

impl SyncedPath {
    fn local_file(&self, repo: &Repository) -> PathBuf {
        let prefix = &repo.get_prefix(self.prefix_id).local;
        prefix.join(&self.path)
    }

    fn remote_file(&self, repo: &Repository) -> String {
        let prefix = &repo.get_prefix(self.prefix_id).remote;
        let mut remote_path = prefix
            .trim_end_matches(std::path::MAIN_SEPARATOR)
            .to_owned();
        remote_path.push_str(&self.path);
        remote_path
    }

    fn from_local(local: &Path, repo: &Repository) -> Self {
        let (prefix_id, path) = repo.split_local_prefix(local);
        SyncedPath {
            prefix_id,
            path: path
                .to_str()
                .expect("non-utf8 characters in path")
                .to_owned(),
        }
    }

    fn from_remote(remote: String, repo: &Repository) -> Self {
        let (prefix_id, path) = repo.split_remote_prefix(&remote);
        SyncedPath {
            prefix_id,
            path: path.to_owned(),
        }
    }
}

struct Tags {
    local: Vec<String>,
    remote: Vec<String>,
}

struct Prefix {
    local: PathBuf,
    remote: String,
}

struct Repository {
    prefixes: Vec<Prefix>,
    files: HashMap<SyncedPath, Tags>,
}

impl Repository {
    fn get_prefix(&self, id: PrefixId) -> &Prefix {
        &self.prefixes[id.0]
    }

    fn split_local_prefix<'a>(&self, file: &'a Path) -> (PrefixId, &'a Path) {
        self.prefixes
            .iter()
            .enumerate()
            .find_map(|(i, Prefix { local, .. })| {
                file.strip_prefix(local)
                    .map(|suffix| (PrefixId(i), suffix))
                    .ok()
            })
            .unwrap_or_else(|| panic!("missing prefix for {}", file.display()))
    }

    fn split_remote_prefix<'a>(&self, file: &'a str) -> (PrefixId, &'a str) {
        self.prefixes
            .iter()
            .enumerate()
            .find_map(|(i, Prefix { remote, .. })| {
                file.strip_prefix(remote)
                    .map(|suffix| (PrefixId(i), suffix))
            })
            .unwrap_or_else(|| panic!("missing prefix for {file}"))
    }

    pub fn insert(&mut self, file: SyncedPath) {}
}
