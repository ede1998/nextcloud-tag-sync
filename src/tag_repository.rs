use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::io::Write;
use std::iter::Peekable;
use std::ops::Deref;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use atomic_write_file::AtomicWriteFile;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use snafu::{IntoError, OptionExt, ResultExt, Snafu, ensure};
use tracing::error;

use crate::{Modification, newtype};

newtype!(PrefixMappingId, usize);

pub const FILE_PATH_ENCODING_SET: AsciiSet = NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'(')
    .remove(b')')
    .remove(b'-')
    .remove(b'.')
    .remove(b'-')
    .remove(b'_');

#[derive(Debug, Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct SyncedPath {
    prefix_id: PrefixMappingId,
    path: PathBuf,
}

impl SyncedPath {
    #[cfg(any(test, feature = "fuzzing"))]
    #[must_use]
    pub fn new(prefix_id: usize, path: &str) -> Self {
        Self {
            prefix_id: PrefixMappingId(prefix_id),
            path: PathBuf::from(path),
        }
    }

    #[must_use]
    pub fn local_file(&self, prefixes: &[PrefixMapping]) -> PathBuf {
        prefixes[self.prefix_id.0].local.join(&self.path)
    }

    #[must_use]
    pub fn remote_file(&self, prefixes: &[PrefixMapping]) -> PathBuf {
        let path = prefixes[self.prefix_id.0].remote.join(&self.path);
        percent_encoding::percent_encode(
            path.as_os_str().as_encoded_bytes(),
            &FILE_PATH_ENCODING_SET,
        )
        .collect()
    }

    #[must_use]
    pub fn relative(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn root(&self) -> PrefixMappingId {
        self.prefix_id
    }

    fn from_local(local: &Path, repo: &Repository) -> Result<Self, MissingPrefix> {
        let (prefix_id, path) = repo.split_prefix(local, FileLocation::Local)?;
        Ok(Self {
            prefix_id,
            path: path.to_owned(),
        })
    }

    fn from_remote(remote: &Path, repo: &Repository) -> Result<Self, MissingPrefix> {
        let binding = Cow::from(percent_encoding::percent_decode(
            remote.as_os_str().as_encoded_bytes(),
        ));
        let remote = OsStr::from_bytes(&binding).as_ref();
        let (prefix_id, path) = repo.split_prefix(remote, FileLocation::Remote)?;
        Ok(Self {
            prefix_id,
            path: path.to_owned(),
        })
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

        impl serde::de::Visitor<'_> for Visitor {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDiff {
    pub identical: Tags,
    pub left_only: Tags,
    pub right_only: Tags,
}

impl TagDiff {
    pub const fn new(removed: Tags, unchanged: Tags, added: Tags) -> Self {
        Self {
            identical: unchanged,
            left_only: removed,
            right_only: added,
        }
    }

    pub const fn removed(&self) -> &Tags {
        &self.left_only
    }

    pub const fn added(&self) -> &Tags {
        &self.right_only
    }

    pub const fn unchanged(&self) -> &Tags {
        &self.identical
    }
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

impl std::fmt::Display for CharacterPrintHelper<'_> {
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
            .inspect_err(|err| error!("Invalid tag name '{s}': {err}"))
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

impl FromIterator<Tag> for Tags {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Tag>,
    {
        Self(iter.into_iter().collect())
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

    #[must_use]
    pub fn diff(&self, Self(right): &Self) -> TagDiff {
        let left = &self.0;
        TagDiff {
            identical: Self(left & right),
            left_only: Self(left - right),
            right_only: Self(right - left),
        }
    }

    pub fn insert_all(&mut self, source: Self) {
        self.0.extend(source.0);
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

#[derive(Debug, Clone, Snafu)]
#[snafu(display("Repo does not have a valid prefix for the given file {}", file.display()))]
pub struct MissingPrefix {
    file: PathBuf,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
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
    pub const fn files(&self) -> &BTreeMap<SyncedPath, Tags> {
        &self.files
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
    ) -> Result<(PrefixMappingId, &'a Path), MissingPrefix> {
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
            .with_context(|| MissingPrefixSnafu {
                file: file.to_path_buf(),
            })
    }

    /// Add a file to the tag repository based on its local path.
    ///
    /// # Errors
    ///
    /// This function will return an error if the path does not have a valid prefix.
    pub fn insert_local(&mut self, path: &Path, tags: Tags) -> Result<(), MissingPrefix> {
        let path = SyncedPath::from_local(path, self)?;
        self.insert(path, tags);
        Ok(())
    }

    /// Add a file to the tag repository based on its remote path.
    ///
    /// # Errors
    ///
    /// This function will return an error if the path does not have a valid prefix.
    pub fn insert_remote(&mut self, path: &Path, tags: Tags) -> Result<SyncedPath, MissingPrefix> {
        let path = SyncedPath::from_remote(path, self)?;
        self.insert(path.clone(), tags);
        Ok(path)
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
    pub fn diff<'collection>(
        &'collection self,
        other: &'collection Self,
    ) -> DiffIterator<'collection> {
        assert_eq!(self.prefixes, other.prefixes);
        DiffIterator::new(self.files.iter(), other.files.iter())
    }

    /// Applies the given difference hunks to the repository.
    ///
    /// # Panics
    ///
    /// If the hunk content conflicts with the repository state.
    pub fn patch(&mut self, hunks: impl IntoIterator<Item = DiffResult>) {
        for DiffResult { path, tags } in hunks {
            let reconstructed_tags = Tags(&tags.identical.0 | &tags.left_only.0);
            let mut result_tags = tags.identical;
            result_tags.insert_all(tags.right_only);

            let old_tags = self
                .files
                .insert(path.clone(), result_tags.clone())
                .unwrap_or_default();
            assert_eq!(
                old_tags, reconstructed_tags,
                "Conflict while applying patch to tag repository: old_tags != reconstructed_tags"
            );
        }
    }

    pub fn rollback_commands(&mut self, commands: impl IntoIterator<Item = crate::Command>) {
        use std::collections::btree_map::Entry;
        for cmd in commands {
            match self.files.entry(cmd.path) {
                Entry::Vacant(entry) => {
                    let tags: Tags = cmd
                        .actions
                        .into_iter()
                        .filter(|a| a.modification == Modification::Remove)
                        .map(|a| a.tag)
                        .collect();
                    entry.insert(tags);
                }
                Entry::Occupied(mut entry) => {
                    let tags = entry.get_mut();
                    for action in cmd.actions {
                        match action.modification {
                            Modification::Add => tags.remove_one(&action.tag),
                            Modification::Remove => tags.insert_one(action.tag),
                        }
                    }
                    if tags.is_empty() {
                        entry.remove();
                    }
                }
            }
        }
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

    #[must_use]
    pub fn stats(&self) -> Statistics {
        use itertools::Itertools;

        fn widen(num: usize) -> u64 {
            num.try_into().expect("num must be less than u64::MAX")
        }

        let files = widen(self.files.len());
        let tags = self.files.values().map(|t| widen(t.len())).sum();
        let distinct_tags = widen(self.files.values().flat_map(|t| &t.0).unique().count());
        let max_tags_on_single_file = widen(
            self.files
                .values()
                .map(|t| t.len())
                .max()
                .unwrap_or_default(),
        );

        Statistics {
            files,
            tags,
            distinct_tags,
            max_tags_on_single_file,
        }
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
pub struct DiffIterator<'collection> {
    left: Peekable<MapIter<'collection>>,
    right: Peekable<MapIter<'collection>>,
}

impl Iterator for DiffIterator<'_> {
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

type MapIter<'collection> = std::collections::btree_map::Iter<'collection, SyncedPath, Tags>;

impl<'collection> DiffIterator<'collection> {
    pub fn new(left: MapIter<'collection>, right: MapIter<'collection>) -> Self {
        Self {
            left: left.peekable(),
            right: right.peekable(),
        }
    }

    fn advance(&mut self, side: Side) -> Option<DiffResult> {
        let (path, left_tags, right_tags) = match side {
            Side::Left => {
                let (path, left) = self.left.next()?;
                (path, left, &Tags::new())
            }
            Side::Right => {
                let (path, right) = self.right.next()?;
                (path, &Tags::new(), right)
            }
            Side::Both => {
                let (path, left) = self.left.next()?;
                let (same_path, right) = self.right.next()?;
                assert_eq!(path, same_path, "Path should be the same");
                (path, left, right)
            }
        };

        let diff = left_tags.diff(right_tags);
        let is_different = !diff.left_only.is_empty() || !diff.right_only.is_empty();

        is_different.then(|| DiffResult {
            path: path.clone(),
            tags: diff,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffResult {
    pub path: SyncedPath,
    pub tags: TagDiff,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
    Both,
}

#[derive(Debug, Clone, Copy)]
pub struct Statistics {
    pub files: u64,
    pub tags: u64,
    pub distinct_tags: u64,
    pub max_tags_on_single_file: u64,
}

impl std::fmt::Display for Statistics {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Self {
            files,
            tags,
            distinct_tags,
            max_tags_on_single_file,
        } = self;
        write!(
            f,
            "Statistics:\nFiles: {files} (most tags on single file = {max_tags_on_single_file})\nTags: {tags} (distinct = {distinct_tags})"
        )
    }
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

        let diff_results_actual: Vec<_> = local_repo.diff(&remote_repo).collect();
        let diff_results_expected: Vec<_> = files
            .iter()
            .filter(|(_, _, local, remote)| !local.is_empty() || !remote.is_empty())
            .collect();

        assert_eq!(diff_results_actual.len(), diff_results_expected.len());
        for (actual, expected) in std::iter::zip(diff_results_actual, diff_results_expected) {
            let (file_path, _, left_only, right_only) = expected;
            assert_eq!(actual.tags.left_only, left_only.iter().copied().collect());
            assert_eq!(actual.tags.right_only, right_only.iter().copied().collect());
            assert_eq!(actual.path, *file_path);
        }
    }

    #[test]
    fn compute_new_repo() {
        let prefixes = mock_prefixes();
        let files = mock_files();

        let mut initial = make_repo(prefixes.clone(), &files, false);
        let modified = make_repo(prefixes.clone(), &files, true);

        let diffs: Vec<_> = initial.diff(&modified).collect();
        initial.patch(diffs);
        println!("{initial:?}");
        assert_eq!(initial.prefixes, prefixes);
        for (actual, expected) in std::iter::zip(initial.files, files) {
            let (path, combined, _initial, modified) = expected;
            let tags = combined.into_iter().chain(modified).collect();

            assert_eq!(actual.1, tags, "Failed for file {}", path.path.display());
        }
    }
}
