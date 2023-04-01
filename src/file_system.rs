use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashSet},
    fmt::Debug,
    iter::Peekable,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct SyncedPath<'prefix> {
    prefix: &'prefix PrefixMapping,
    path: PathBuf,
}

impl<'prefix> SyncedPath<'prefix> {
    pub fn local_file(&self) -> PathBuf {
        self.prefix.local.join(&self.path)
    }

    pub fn remote_file(&self) -> PathBuf {
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

struct TagDiff {
    identical: Tags,
    left_only: Tags,
    right_only: Tags,
}

#[derive(Clone)]
pub struct Tags(HashSet<String>);

impl Debug for Tags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.0.iter()).finish()
    }
}

impl Tags {
    fn new() -> Self {
        Tags(HashSet::new())
    }

    fn diff(self, Tags(mut right): Self) -> TagDiff {
        let mut left = HashSet::new();
        let mut both = HashSet::new();

        for tag in self.0 {
            if right.take(&tag).is_none() {
                left.insert(tag);
            } else {
                both.insert(tag);
            }
        }

        TagDiff {
            identical: Tags(both),
            left_only: Tags(left),
            right_only: Tags(right),
        }
    }

    fn insert_all(&mut self, source: &Tags) {
        self.0.extend(source.0.iter().cloned());
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct PrefixMapping {
    local: PathBuf,
    remote: PathBuf,
}

pub struct Repository<'prefix> {
    prefixes: &'prefix [PrefixMapping],
    files: BTreeMap<SyncedPath<'prefix>, Tags>,
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

    pub fn diff(self, other: Self, keep_side_on_conflict: Side) -> DiffIterator<'prefix> {
        assert_eq!(self.prefixes, other.prefixes);
        DiffIterator::new(
            self.files.into_iter(),
            other.files.into_iter(),
            self.prefixes,
            keep_side_on_conflict,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileLocation {
    Local,
    Remote,
}

pub struct DiffIterator<'prefix> {
    left: Peekable<MapIter<'prefix>>,
    right: Peekable<MapIter<'prefix>>,
    prefixes: &'prefix [PrefixMapping],
    files: BTreeMap<SyncedPath<'prefix>, Tags>,
    keep_side_on_conflict: Side,
}

impl<'prefix> Iterator for DiffIterator<'prefix> {
    type Item = DiffResult<'prefix>;

    fn next(&mut self) -> Option<Self::Item> {
        let (left, right) = match (self.left.peek(), self.right.peek()) {
            (None, None) => return None,
            (None, Some(_)) => {
                return self.advance(Side::Right);
            }
            (Some(_), None) => {
                return self.advance(Side::Left);
            }
            (Some(l), Some(r)) => (&l.0, &r.0),
        };
        match left.cmp(right) {
            Ordering::Less => {
                return self.advance(Side::Left);
            }
            Ordering::Greater => {
                return self.advance(Side::Right);
            }
            Ordering::Equal => {
                return self.advance(Side::Both);
            }
        }
    }
}

type MapIter<'a> = std::collections::btree_map::IntoIter<SyncedPath<'a>, Tags>;

impl<'prefix> DiffIterator<'prefix> {
    pub fn new(
        left: MapIter<'prefix>,
        right: MapIter<'prefix>,
        prefixes: &'prefix [PrefixMapping],
        keep_side_on_conflict: Side,
    ) -> Self {
        DiffIterator {
            left: left.peekable(),
            right: right.peekable(),
            prefixes,
            files: BTreeMap::new(),
            keep_side_on_conflict,
        }
    }

    pub fn finish(self) -> Repository<'prefix> {
        Repository {
            prefixes: self.prefixes,
            files: self.files,
        }
    }

    fn diff_tags(&mut self, left: Tags, right: Tags, path: SyncedPath<'prefix>) -> (Tags, Tags) {
        let diff = left.diff(right);
        let mut result_tags = diff.identical;

        match self.keep_side_on_conflict {
            Side::Left => {
                result_tags.insert_all(&diff.left_only);
            }
            Side::Right => {
                result_tags.insert_all(&diff.right_only);
            }
            Side::Both => {
                result_tags.insert_all(&diff.left_only);
                result_tags.insert_all(&diff.right_only);
            }
        }

        self.files.insert(path, result_tags);

        (diff.left_only, diff.right_only)
    }

    fn advance(&mut self, side: Side) -> Option<DiffResult<'prefix>> {
        let ((same_path, left_tags), (path, right_tags)) = match side {
            Side::Left => {
                let (path, left) = self.left.next()?;
                ((path.clone(), left), (path, Tags::new()))
            }
            Side::Right => {
                let (path, right) = self.right.next()?;
                ((path.clone(), Tags::new()), (path, right))
            }
            Side::Both => (self.left.next()?, self.right.next()?),
        };

        let (left_only, right_only) = self.diff_tags(left_tags, right_tags, same_path);

        Some(DiffResult {
            path,
            left_only,
            right_only,
        })
    }
}

pub struct DiffResult<'prefix> {
    path: SyncedPath<'prefix>,
    left_only: Tags,
    right_only: Tags,
}

#[derive(Clone, Copy, Debug)]
pub enum Side {
    Left,
    Right,
    Both,
}
