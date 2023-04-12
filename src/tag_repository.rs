use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::convert::Infallible;
use std::fmt::Debug;
use std::iter::Peekable;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
struct PrefixMappingId(usize);

#[derive(Debug, Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct SyncedPath {
    prefix_id: PrefixMappingId,
    path: PathBuf,
}

impl SyncedPath {
    #[cfg(test)]
    pub fn new(prefix_id: usize, path: &str) -> Self {
        Self {
            prefix_id: PrefixMappingId(prefix_id),
            path: PathBuf::from(path),
        }
    }

    pub fn local_file(&self, repo: &Repository) -> PathBuf {
        repo.prefixes[self.prefix_id.0].local.join(&self.path)
    }

    pub fn remote_file(&self, repo: &Repository) -> PathBuf {
        repo.prefixes[self.prefix_id.0].remote.join(&self.path)
    }

    fn from_local(local: &Path, repo: &Repository) -> Self {
        let (prefix_id, path) = repo.split_prefix(local, FileLocation::Local);
        SyncedPath {
            prefix_id,
            path: path.to_owned(),
        }
    }

    fn from_remote(remote: &Path, repo: &Repository) -> Self {
        let (prefix_id, path) = repo.split_prefix(remote, FileLocation::Remote);
        SyncedPath {
            prefix_id,
            path: path.to_owned(),
        }
    }
}

struct TagDiff {
    identical: Tags,
    left_only: Tags,
    right_only: Tags,
}

// TODO introduce Tag newtype for validation: only A-Z a-z 0-9 and - allowed
#[derive(Clone, PartialEq, Eq)]
pub struct Tags(HashSet<String>);

impl FromStr for Tags {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tags = s
            .split(',')
            .filter(|s| !s.is_empty())
            .map(Into::into)
            .collect();
        Ok(Self(tags))
    }
}

impl std::fmt::Display for Tags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let tags = self.0.iter().map(Deref::deref).collect::<Vec<_>>().join(",");
        f.write_str(&tags)
    }
}

impl FromIterator<String> for Tags {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = String>,
    {
        Tags(iter.into_iter().collect())
    }
}

impl<'a> FromIterator<&'a str> for Tags {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = &'a str>,
    {
        Tags(iter.into_iter().map(ToOwned::to_owned).collect())
    }
}

impl Deref for Tags {
    type Target = HashSet<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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

    pub fn insert_all(&mut self, source: &Tags) {
        self.0.extend(source.0.iter().cloned());
    }

    pub fn insert_one(&mut self, tag: String) {
        self.0.insert(tag);
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PrefixMapping {
    local: PathBuf,
    remote: PathBuf,
}

impl PrefixMapping {
    pub fn new(local: PathBuf, remote: PathBuf) -> Self {
        Self { local, remote }
    }

    pub fn local(&self) -> &Path {
        &self.local
    }

    pub fn remote(&self) -> &Path {
        &self.remote
    }
}

#[derive(Clone, Debug)]
pub struct Repository {
    prefixes: Vec<PrefixMapping>,
    files: BTreeMap<SyncedPath, Tags>,
}

impl Repository {
    pub fn new(prefixes: Vec<PrefixMapping>) -> Self {
        Self {
            prefixes,
            files: BTreeMap::new(),
        }
    }

    fn split_prefix<'a>(
        &self,
        file: &'a Path,
        location: FileLocation,
    ) -> (PrefixMappingId, &'a Path) {
        self.prefixes
            .iter()
            .enumerate()
            .find_map(|(i, prefix_map)| {
                let prefix = match location {
                    FileLocation::Local => &prefix_map.local,
                    FileLocation::Remote => &prefix_map.remote,
                };
                file.strip_prefix(prefix)
                    .map(|suffix| (PrefixMappingId(i), suffix))
                    .ok()
            })
            .unwrap_or_else(|| panic!("missing prefix for {}", file.display()))
    }

    pub fn insert_local(&mut self, path: &Path, tags: Tags) {
        let path = SyncedPath::from_local(path, self);
        self.insert(path, tags);
    }

    pub fn insert_remote(&mut self, path: &Path, tags: Tags) {
        let path = SyncedPath::from_remote(path, self);
        self.insert(path, tags);
    }

    pub fn insert(&mut self, path: SyncedPath, tags: Tags) {
        self.files.insert(path, tags);
    }

    pub fn diff(self, other: Self, keep_side_on_conflict: Side) -> DiffIterator {
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

pub struct DiffIterator {
    left: Peekable<MapIter>,
    right: Peekable<MapIter>,
    prefixes: Vec<PrefixMapping>,
    files: BTreeMap<SyncedPath, Tags>,
    keep_side_on_conflict: Side,
}

impl Iterator for DiffIterator {
    type Item = DiffResult;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
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
                    let next = self.advance(Side::Both);
                    if next.is_some() {
                        return next;
                    }
                }
            }
        }
    }
}

type MapIter = std::collections::btree_map::IntoIter<SyncedPath, Tags>;

impl DiffIterator {
    pub fn new(
        left: MapIter,
        right: MapIter,
        prefixes: Vec<PrefixMapping>,
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

    pub fn finish(mut self) -> Repository {
        // exhaust iterator if not already exhausted
        (&mut self).for_each(drop);
        Repository {
            prefixes: self.prefixes,
            files: self.files,
        }
    }

    fn diff_tags(&mut self, left: Tags, right: Tags, path: SyncedPath) -> (Tags, Tags) {
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

    fn advance(&mut self, side: Side) -> Option<DiffResult> {
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
        let is_different = !left_only.is_empty() || !right_only.is_empty();

        is_different.then_some(DiffResult {
            path,
            left_only,
            right_only,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DiffResult {
    pub path: SyncedPath,
    pub left_only: Tags,
    pub right_only: Tags,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
    Both,
}

#[cfg(test)]
mod tests {
    use super::*;

    type TaggedFile = (
        SyncedPath,
        Vec<&'static str>,
        Vec<&'static str>,
        Vec<&'static str>,
    );

    fn mock_prefixes() -> Vec<PrefixMapping> {
        vec![
            PrefixMapping {
                local: "/local/one".into(),
                remote: "/remote/one".into(),
            },
            PrefixMapping {
                local: "/local/two".into(),
                remote: "/remote/two".into(),
            },
        ]
    }

    fn mock_files() -> Vec<TaggedFile> {
        #[rustfmt::skip]
        let files = vec![
            (SyncedPath::new(0, "fumbling/driver"      ), vec!["fog", "error"],          vec!["study"], vec!["sheet", "toilet", "time"]),
            (SyncedPath::new(0, "gruesome/tourney"     ), vec!["stop", "event"],         vec![],        vec![]),
            (SyncedPath::new(0, "outstanding/solemnity"), vec!["mark", "jelly", "team"], vec!["brain"], vec![]),
            (SyncedPath::new(0, "succinct/watchdog"    ), vec!["fog", "error", "sheet"], vec![],        vec![]),
            (SyncedPath::new(1, "clueless/lodging"     ), vec![],                        vec!["burn"],  vec![]),
            (SyncedPath::new(1, "experienced/mission"  ), vec![],                        vec![],        vec!["pull"]),
            (SyncedPath::new(1, "grand/appraisal"      ), vec!["plastic", "dinosaurs"],  vec![],        vec![]),
            (SyncedPath::new(1, "tight/earnings"       ), vec!["tree", "forest"],        vec![],        vec![]),
        ];

        files
    }

    fn make_repo<'a, I: IntoIterator<Item = &'a TaggedFile>>(
        prefixes: Vec<PrefixMapping>,
        iter: I,
        use_remote_files: bool,
    ) -> Repository {
        let mut repo = Repository::new(prefixes);
        for (file_path, combined, local, remote) in iter {
            let tags = if use_remote_files {
                remote.iter()
            } else {
                local.iter()
            }
            .chain(combined.iter())
            .copied()
            .collect();

            repo.insert(file_path.clone(), tags);
        }

        repo
    }

    #[test]
    fn compute_diff_results() {
        let prefixes = mock_prefixes();
        let files = mock_files();

        let local_repo = make_repo(prefixes.clone(), &files, false);
        let remote_repo = make_repo(prefixes, &files, true);

        let mut diffs = local_repo.diff(remote_repo, Side::Both);

        let diff_results_actual: Vec<_> = (&mut diffs).collect();
        let diff_results_expected: Vec<_> = files
            .iter()
            .filter(|(_, _, local, remote)| !local.is_empty() || !remote.is_empty())
            .collect();

        assert_eq!(diff_results_actual.len(), diff_results_expected.len());
        for (actual, expected) in std::iter::zip(diff_results_actual, diff_results_expected) {
            let (file_path, _, left_only, right_only) = expected;
            assert_eq!(actual.left_only, left_only.iter().copied().collect());
            assert_eq!(actual.right_only, right_only.iter().copied().collect());
            assert_eq!(actual.path, *file_path);
        }
    }

    #[test]
    fn compute_new_repo_with_both() {
        compute_new_repo(Side::Both);
    }

    #[test]
    fn compute_new_repo_with_left() {
        compute_new_repo(Side::Left);
    }

    #[test]
    fn compute_new_repo_with_right() {
        compute_new_repo(Side::Right);
    }

    fn compute_new_repo(keep_action: Side) {
        let prefixes = mock_prefixes();
        let files = mock_files();

        let local_repo = make_repo(prefixes.clone(), &files, false);
        let remote_repo = make_repo(prefixes.clone(), &files, true);

        let diffs = local_repo.diff(remote_repo, keep_action);
        let new_repo = diffs.finish();
        println!("{new_repo:?}");
        assert_eq!(new_repo.prefixes, prefixes);
        for (actual, expected) in std::iter::zip(new_repo.files, files) {
            let (path, combined, local, remote) = expected;
            let tags = match keep_action {
                Side::Left => combined.into_iter().chain(local).collect(),
                Side::Right => combined.into_iter().chain(remote).collect(),
                Side::Both => combined.into_iter().chain(local).chain(remote).collect(),
            };

            assert_eq!(actual.1, tags, "Failed for file {}", path.path.display());
        }
    }
}
