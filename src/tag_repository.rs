use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::fmt::Debug;
use std::io::Write;
use std::iter::Peekable;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use snafu::{ensure, IntoError, ResultExt, Snafu};
use tracing::error;

use crate::newtype;

newtype!(PrefixMappingId, usize);

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

    pub fn local_file(&self, prefixes: &[PrefixMapping]) -> PathBuf {
        prefixes[self.prefix_id.0].local.join(&self.path)
    }

    pub fn remote_file(&self, prefixes: &[PrefixMapping]) -> PathBuf {
        prefixes[self.prefix_id.0].remote.join(&self.path)
    }

    pub fn relative(&self) -> &Path {
        &self.path
    }

    pub const fn root(&self) -> PrefixMappingId {
        self.prefix_id
    }

    fn from_local(local: &Path, repo: &Repository) -> Self {
        let (prefix_id, path) = repo.split_prefix(local, FileLocation::Local);
        Self {
            prefix_id,
            path: path.to_owned(),
        }
    }

    fn from_remote(remote: &Path, repo: &Repository) -> Self {
        let (prefix_id, path) = repo.split_prefix(remote, FileLocation::Remote);
        Self {
            prefix_id,
            path: path.to_owned(),
        }
    }
}

impl Serialize for SyncedPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let Some(path) = self.path.to_str() else {
            return Err(serde::ser::Error::custom(
                "path contains invalid UTF-8 characters",
            ));
        };
        let id = self.prefix_id.0;
        serializer.serialize_str(&format!("{id}:{path}"))
    }
}

impl<'de> Deserialize<'de> for SyncedPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = SyncedPath;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a path in the form '<ID>:/path/to/the/file' where ID is the prefix mapping number")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let Some((prefix_id, path)) = v.split_once(':') else {
                    return Err(serde::de::Error::custom("Missing ':' in SyncedPath"));
                };

                Ok(SyncedPath {
                    prefix_id: prefix_id.parse().map_err(|_| {
                        serde::de::Error::custom("Prefix mapping id was not a number")
                    })?,
                    path: path.into(),
                })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl std::fmt::Display for SyncedPath {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "/[ID-{}]/{}", self.prefix_id, self.path.display())
    }
}

struct TagDiff {
    identical: Tags,
    left_only: Tags,
    right_only: Tags,
}

#[derive(Debug, Snafu)]
pub enum TagParseError {
    #[snafu(display(
        "some characters not allowed in tag: {}",
        CharacterPrintHelper(invalid)
    ))]
    InvalidCharacters { invalid: Vec<(usize, char)> },
    #[snafu(display("tag may not be empty"))]
    EmptyTag,
}

struct CharacterPrintHelper<'a>(&'a [(usize, char)]);

impl<'a> std::fmt::Display for CharacterPrintHelper<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut next = self.0.iter().peekable();
        while let Some((position, character)) = next.next() {
            write!(f, "{character} at {position}")?;
            if next.peek().is_some() {
                f.write_str(", ")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Tag(String);

impl Tag {
    pub(crate) fn new_or_log_error(s: &str) -> Option<Self> {
        s.parse()
            .map_err(|err| {
                error!("Invalid tag name '{s}': {err}");
                err
            })
            .ok()
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Deref for Tag {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for Tag {
    type Err = TagParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ensure!(!s.is_empty(), EmptyTagSnafu);

        let invalid: Vec<_> = s
            .chars()
            .enumerate()
            .filter(|(_, c)| !c.is_alphanumeric() && !"-–.' _".contains(*c))
            .collect();

        ensure!(invalid.is_empty(), InvalidCharactersSnafu { invalid });

        Ok(Self(s.to_owned()))
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tags(BTreeSet<Tag>);

impl FromStr for Tags {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tags = if s.is_empty() {
            BTreeSet::default()
        } else {
            s.split(',').filter_map(Tag::new_or_log_error).collect()
        };
        Ok(Self(tags))
    }
}

impl<const N: usize> From<[Tag; N]> for Tags {
    fn from(value: [Tag; N]) -> Self {
        Self(value.into_iter().collect())
    }
}

impl std::fmt::Display for Tags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let tags = self
            .0
            .iter()
            .map(Deref::deref)
            .collect::<Vec<_>>()
            .join(",");
        f.write_str(&tags)
    }
}

impl IntoIterator for Tags {
    type Item = Tag;

    type IntoIter = std::collections::btree_set::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromIterator<String> for Tags {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = String>,
    {
        Self(
            iter.into_iter()
                .filter_map(|t| Tag::new_or_log_error(&t))
                .collect(),
        )
    }
}

impl<'a> FromIterator<&'a str> for Tags {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = &'a str>,
    {
        Self(iter.into_iter().filter_map(Tag::new_or_log_error).collect())
    }
}

impl Deref for Tags {
    type Target = BTreeSet<Tag>;

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
    const fn new() -> Self {
        Self(BTreeSet::new())
    }

    fn diff(self, Self(mut right): Self) -> TagDiff {
        let mut left = BTreeSet::new();
        let mut both = BTreeSet::new();

        for tag in self.0 {
            if right.take(&tag).is_none() {
                left.insert(tag);
            } else {
                both.insert(tag);
            }
        }

        TagDiff {
            identical: Self(both),
            left_only: Self(left),
            right_only: Self(right),
        }
    }

    pub fn insert_all(&mut self, source: &Self) {
        self.0.extend(source.0.iter().cloned());
    }

    pub fn insert_one(&mut self, tag: Tag) {
        self.0.insert(tag);
    }

    pub fn remove_one(&mut self, tag: &Tag) {
        self.0.remove(tag);
    }
}

fn deserialize_remote_path<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: serde::Deserializer<'de>,
    D::Error: serde::de::Error,
{
    let path = PathBuf::deserialize(deserializer)?;
    if path.starts_with(PrefixMapping::EXPECTED_PREFIX) {
        Ok(path)
    } else {
        Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Bytes(path.as_os_str().as_encoded_bytes()),
            // Sadly, I would need an extra dependency to concat string constants at compile time
            &"a string starting with /remote.php/dav/files/",
        ))
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PrefixMapping {
    local: PathBuf,
    #[serde(deserialize_with = "deserialize_remote_path")]
    remote: PathBuf,
}

impl PrefixMapping {
    /// Constructs a new prefix mapping.
    ///
    /// # Errors
    ///
    /// This function will return an error if `remote` does not start with /remote.php/dav/files/.
    pub fn new(local: PathBuf, remote: PathBuf) -> Result<Self, &'static str> {
        if remote.starts_with(Self::EXPECTED_PREFIX) {
            Ok(Self { local, remote })
        } else {
            Err("Remote path must start with /remote.php/dav/files/")
        }
    }

    #[must_use]
    pub fn local(&self) -> &Path {
        &self.local
    }

    #[must_use]
    pub fn remote(&self) -> &Path {
        &self.remote
    }

    pub const EXPECTED_PREFIX: &str = "/remote.php/dav/files/";
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Repository {
    prefixes: Vec<PrefixMapping>,
    files: BTreeMap<SyncedPath, Tags>,
}

impl Repository {
    #[must_use]
    pub const fn new(prefixes: Vec<PrefixMapping>) -> Self {
        Self {
            prefixes,
            files: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn validate_prefix_mapping(&self, expected: &[PrefixMapping]) -> bool {
        let prefix_count = self.prefixes.len();

        if prefix_count > expected.len() {
            return false;
        }

        self.prefixes == expected[..prefix_count]
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

    pub fn insert_remote(&mut self, path: &Path, tags: Tags) -> SyncedPath {
        let path = SyncedPath::from_remote(path, self);
        self.insert(path.clone(), tags);
        path
    }

    pub fn insert(&mut self, path: SyncedPath, tags: Tags) {
        self.files.insert(path, tags);
    }

    /// Computes the differences between self and other file tag repository.
    ///
    /// # Panics
    ///
    /// This function panics if the synchronization prefixes between the repositories
    /// don't match. In this case, the results would be garbage.
    #[must_use]
    pub fn diff(self, other: Self, keep_side_on_conflict: Side) -> DiffIterator {
        assert_eq!(self.prefixes, other.prefixes);
        DiffIterator::new(
            self.files.into_iter(),
            other.files.into_iter(),
            self.prefixes,
            keep_side_on_conflict,
        )
    }

    /// Store the repository on disk in json format.
    ///
    /// # Errors
    ///
    /// This function will return an error if serialization or write process fails.
    pub fn persist_on_disk(&self, path: &Path) -> Result<(), PersistingError> {
        tracing::info!("Persisting repository to disk at {}", path.display());
        let result = serde_json::to_string_pretty(self).context(SerializationSnafu)?;
        let mut file = AtomicWriteFile::open(path).with_context(|_| OpenSnafu { path })?;
        file.write_all(result.as_ref())
            .with_context(|_| WriteSnafu { path })?;
        file.commit().with_context(|_| OpenSnafu { path })?;
        Ok(())
    }

    /// Read the repository from disk in json format.
    ///
    /// # Errors
    ///
    /// This function will return an error if the read process or deserialization fails.
    pub fn read_from_disk(path: &Path) -> Result<Self, LoadError> {
        tracing::info!("Reading repository from disk at {}", path.display());
        let data = std::fs::read_to_string(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => NotFoundSnafu { path }.into_error(snafu::NoneError),
            _ => IoSnafu { path }.into_error(e),
        })?;
        let repo = serde_json::from_str(&data).with_context(|_| DeserializationSnafu { path })?;
        Ok(repo)
    }
}

#[derive(Snafu, Debug)]
pub enum LoadError {
    #[snafu(display("failed to deserialize repository from json file {}", path.display()))]
    Deserialization {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[snafu(display("failed to read repository from file"))]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("failed find file {} for reading", path.display()))]
    NotFound { path: PathBuf },
}

#[derive(Snafu, Debug)]
pub enum PersistingError {
    #[snafu(display("failed to serialize repository as json"))]
    Serialization { source: serde_json::Error },
    #[snafu(display("failed to open file {}", path.display()))]
    Open {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("failed to write repository data to file {}", path.display()))]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileLocation {
    Local,
    Remote,
}

#[derive(Debug)]
pub struct DiffIterator {
    left: Peekable<MapIter>,
    right: Peekable<MapIter>,
    prefixes: Vec<PrefixMapping>,
    files: BTreeMap<SyncedPath, Tags>,
    pub source_of_truth: Side,
}

impl Iterator for &mut DiffIterator {
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
        source_of_truth: Side,
    ) -> Self {
        Self {
            left: left.peekable(),
            right: right.peekable(),
            prefixes,
            files: BTreeMap::new(),
            source_of_truth,
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

        match self.source_of_truth {
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
