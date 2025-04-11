use std::sync::Arc;

use snafu::Snafu;

use crate::{
    Command, CommandsFormatter, Config, FileSystem, ListTagsError, LocalError, LocalFs, RemoteFs,
    Repository, resolve_diffs,
    tag_repository::{DiffResult, LoadError, PersistingError, Side, TagDiff},
};

pub struct Uninitialized {
    pub config: Arc<Config>,
    pub remote_fs: RemoteFs,
    pub local_fs: LocalFs,
}

impl Uninitialized {
    #[must_use]
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            remote_fs: RemoteFs::new(config.clone()),
            local_fs: LocalFs::new(config.clone()),
            config,
        }
    }

    async fn create_from_local_remote_diff(mut self) -> Result<Initialized, InitError> {
        let (local, remote) = merge_results(futures::join!(
            self.local_fs.create_repo(),
            self.remote_fs.create_repo()
        ))?;

        let mut initial_repo = match self.config.keep_side_on_conflict {
            Side::Left => local.clone(),
            Side::Right => remote.clone(),
            Side::Both => Repository::new(self.config.prefixes.clone()),
        };

        let (local_actions, remote_actions) = in_memory_patch(&mut initial_repo, &local, &remote);

        if self.config.dry_run {
            tracing::info!("Skipping tag sync because of dry-run");
        } else {
            let fails = futures::join!(
                self.local_fs.update_tags(local_actions),
                self.remote_fs.update_tags(remote_actions)
            );
            handle_failures(&mut initial_repo, fails);
        }

        Ok(Initialized {
            repo: initial_repo,
            remote_fs: self.remote_fs,
            local_fs: self.local_fs,
            config: self.config,
        })
    }

    #[expect(clippy::result_large_err, reason = "Only called once at startup")]
    fn load_from_file(self) -> Result<Initialized, Self> {
        match Repository::read_from_disk(&self.config.tag_database) {
            Ok(repo) if repo.validate_prefix_mapping(&self.config.prefixes) => Ok(Initialized {
                repo,
                local_fs: self.local_fs,
                remote_fs: self.remote_fs,
                config: self.config,
            }),
            Err(LoadError::NotFound { .. }) => {
                tracing::info!("No previous repository exists yet. Starting from scratch.");
                Err(self)
            }
            Ok(_) => {
                tracing::error!(
                    "Repository created for incompatible prefix mapping configuration. Ignoring it."
                );
                Err(self)
            }
            Err(e) => {
                tracing::error!("Failed to load repository file: {e:?}");
                Err(self)
            }
        }
    }

    /// Initialize a file tag repository by loading it from a cache file.
    /// If loading from file fails, e.g. because no cache exists yet, a new
    /// one is built from scratch.
    ///
    /// # Errors
    ///
    /// This function will return an error if initialization fails.
    pub async fn initialize(self) -> Result<Initialized, InitError> {
        match self.load_from_file() {
            Ok(o) => Ok(o),
            Err(this) => this.create_from_local_remote_diff().await,
        }
    }
}

#[derive(Debug)]
pub struct Initialized {
    config: Arc<Config>,
    repo: Repository,
    remote_fs: RemoteFs,
    local_fs: LocalFs,
}

impl Initialized {
    #[must_use]
    pub const fn repository(&self) -> &Repository {
        &self.repo
    }

    /// Computes changes of the local and remote tags compared to the cache and applies to change on the other side as well as updates the internal model.
    ///
    /// # Errors
    ///
    /// This function will return an error if computing either file tag repository fails.
    pub async fn sync(&mut self) -> Result<(), InitError> {
        let (local, remote) = merge_results(futures::join!(
            self.local_fs.create_repo(),
            self.remote_fs.create_repo()
        ))?;

        let (local_actions, remote_actions) = in_memory_patch(&mut self.repo, &local, &remote);

        if self.config.dry_run {
            tracing::info!("Skipping tag sync because of dry-run");
        } else {
            let fails = futures::join!(
                self.local_fs.update_tags(local_actions),
                self.remote_fs.update_tags(remote_actions)
            );
            handle_failures(&mut self.repo, fails);
        }

        Ok(())
    }

    /// Persist the repository to disk.
    ///
    /// # Errors
    ///
    /// This function will return an error if persisting failed.
    pub fn persist_repository(&self) -> Result<(), PersistingError> {
        if self.config.dry_run {
            tracing::info!("Not saving data because of dry-run");
            return Ok(());
        }
        self.repo.persist_on_disk(&self.config.tag_database)
    }
}

pub fn in_memory_patch(
    original_repo: &mut Repository,
    local: &Repository,
    remote: &Repository,
) -> (Vec<Command>, Vec<Command>) {
    let mut local: Vec<_> = original_repo.diff(local).collect();
    let mut remote: Vec<_> = original_repo.diff(remote).collect();

    let identical = filter_identical_modifications(&mut local, &mut remote);

    let local_actions = resolve_diffs(remote.clone());
    let remote_actions = resolve_diffs(local.clone());
    tracing::info!("Remote actions: {}", CommandsFormatter(&remote_actions));
    tracing::info!("Local actions: {}", CommandsFormatter(&local_actions));

    let merged = merge_modifications([identical, local, remote]);
    original_repo.patch(merged);

    (local_actions, remote_actions)
}

fn handle_failures(repo: &mut Repository, fails: (Vec<Command>, Vec<Command>)) {
    let (local, remote) = fails;
    if !local.is_empty() || !remote.is_empty() {
        tracing::info!("Rolling back local fails: {}", CommandsFormatter(&local));
        tracing::info!("Rolling back remote fails: {}", CommandsFormatter(&remote));
    }
    repo.rollback_commands(local.into_iter().chain(remote));
}

fn merge_modifications(diffs: impl IntoIterator<Item = Vec<DiffResult>>) -> Vec<DiffResult> {
    let mut remainder = diffs.into_iter();
    let Some(mut result) = remainder.next() else {
        return Vec::new();
    };

    let comparator = |a: &DiffResult, b: &DiffResult| a.path.cmp(&b.path);
    result.sort_unstable_by(comparator);

    remainder.fold(result, |result, mut other| {
        other.sort_unstable_by(comparator);
        itertools::merge_join_by(result, other, comparator)
            .map(|merged| match merged {
                itertools::EitherOrBoth::Left(r) => r,
                itertools::EitherOrBoth::Right(o) => o,
                itertools::EitherOrBoth::Both(mut r, o) => {
                    r.tags.left_only.insert_all(o.tags.left_only);
                    // Don't add unchanged tags.
                    // r.tags.identical.insert_all(o.tags.identical);
                    r.tags.right_only.insert_all(o.tags.right_only);
                    r
                }
            })
            .collect()
    })
}

fn filter_identical_modifications(
    left: &mut Vec<DiffResult>,
    right: &mut Vec<DiffResult>,
) -> Vec<DiffResult> {
    let comparator = |a: &DiffResult, b: &DiffResult| a.path.cmp(&b.path);

    left.sort_unstable_by(comparator);
    right.sort_unstable_by(comparator);

    let mut identical = Vec::new();

    for merged in itertools::merge_join_by(std::mem::take(left), std::mem::take(right), comparator)
    {
        match merged {
            itertools::EitherOrBoth::Left(l) => left.push(l),
            itertools::EitherOrBoth::Right(r) => right.push(r),
            itertools::EitherOrBoth::Both(l, r) if l == r => identical.push(l),
            itertools::EitherOrBoth::Both(l, r) => {
                let removed = l.tags.removed().diff(r.tags.removed());
                let unchanged = l.tags.unchanged().diff(r.tags.unchanged());
                let added = l.tags.added().diff(r.tags.added());

                left.push(DiffResult {
                    path: l.path,
                    tags: TagDiff::new(removed.left_only, unchanged.left_only, added.left_only),
                });

                identical.push(DiffResult {
                    path: r.path.clone(),
                    tags: TagDiff::new(removed.identical, unchanged.identical, added.identical),
                });

                right.push(DiffResult {
                    path: r.path,
                    tags: TagDiff::new(removed.right_only, unchanged.right_only, added.right_only),
                });
            }
        }
    }

    identical
}

#[allow(clippy::result_large_err)] // only runs once -> no performance issue anyway
fn merge_results<T, U>(
    results: (Result<T, InitError>, Result<U, InitError>),
) -> Result<(T, U), InitError> {
    match results {
        (Ok(l), Ok(r)) => Ok((l, r)),
        (Ok(_), Err(e)) | (Err(e), Ok(_)) => Err(e),
        (
            Err(InitError::Local {
                source: source_local,
            }),
            Err(InitError::Remote {
                source: source_remote,
            }),
        )
        | (
            Err(InitError::Remote {
                source: source_remote,
            }),
            Err(InitError::Local {
                source: source_local,
            }),
        ) => BothSnafu {
            source_local,
            source_remote,
        }
        .fail(),
        _ => panic!("Wrongly called, todo"),
    }
}

#[derive(Snafu, Debug)]
#[snafu(visibility(pub))]
pub enum InitError {
    #[snafu(display("failed to construct local repository"))]
    Local { source: LocalError },
    #[snafu(display("failed to construct remote repository"))]
    Remote { source: ListTagsError },
    #[snafu(display("failed to construct local and remote repository"))]
    Both {
        source_local: LocalError,
        source_remote: ListTagsError,
    },
}
