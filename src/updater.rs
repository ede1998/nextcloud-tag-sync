use std::sync::Arc;

use snafu::Snafu;

use crate::{
    resolve_diffs,
    tag_repository::{LoadError, PersistingError, Side},
    CommandsFormatter, Config, FileSystem, ListTagsError, LocalError, LocalFs, RemoteFs,
    Repository,
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
        let remote_repo_task = self.remote_fs.create_repo();
        let local_repo_task = self.local_fs.create_repo();

        let (local, remote) = merge_results(futures::join!(local_repo_task, remote_repo_task))?;

        let mut diff_events = local.diff(remote, self.config.keep_side_on_conflict);
        let (local_actions, remote_actions) =
            resolve_diffs(&mut diff_events, self.config.keep_side_on_conflict);

        let cmd_fmt = CommandsFormatter(&local_actions);
        println!("{cmd_fmt}");
        let cmd_fmt = CommandsFormatter(&remote_actions);
        println!("{cmd_fmt}");

        self.remote_fs.update_tags(remote_actions).await;
        self.local_fs.update_tags(local_actions).await;

        Ok(Initialized {
            repo: diff_events.finish(),
            remote_fs: self.remote_fs,
            local_fs: self.local_fs,
            config: self.config,
        })
    }

    #[expect(clippy::result_large_err, reason = "Only called once at startup")]
    fn load_from_file(self) -> Result<Initialized, Self> {
        match Repository::read_from_disk(&self.config.tag_database) {
            Ok(repo) => Ok(Initialized {
                repo,
                local_fs: self.local_fs,
                remote_fs: self.remote_fs,
                config: self.config,
            }),
            Err(LoadError::NotFound { .. }) => Err(self),
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

    /// Computes changes of the local tags compared to the cache and uploads all changes to the remote.
    ///
    /// # Errors
    ///
    /// This function will return an error if computing the local file tag repository fails.
    pub async fn sync_local_to_remote(&mut self) -> Result<(), InitError> {
        let local = self.local_fs.create_repo().await?;

        let repo = std::mem::take(&mut self.repo);
        let mut diff_events = repo.diff(local, Side::Right);
        let (_, actions) = resolve_diffs(&mut diff_events, Side::Right);

        let cmd_fmt = CommandsFormatter(&actions);
        println!("{cmd_fmt}");

        self.remote_fs.update_tags(actions).await;
        self.repo = diff_events.finish();
        Ok(())
    }

    /// Computes changes of the remote tags compared to the cache and mirrors all changes to the local filesystem.
    ///
    /// # Errors
    ///
    /// This function will return an error if computing the remote file tag repository fails.
    pub async fn sync_remote_to_local(&mut self) -> Result<(), InitError> {
        let remote = self.remote_fs.create_repo().await?;

        let repo = std::mem::take(&mut self.repo);
        let mut diff_events = repo.diff(remote, Side::Right);
        let (_, actions) = resolve_diffs(&mut diff_events, Side::Right);

        let cmd_fmt = CommandsFormatter(&actions);
        println!("{cmd_fmt}");

        self.local_fs.update_tags(actions).await;

        self.repo = diff_events.finish();
        Ok(())
    }

    /// Persist the repository to disk.
    ///
    /// # Errors
    ///
    /// This function will return an error if persisting failed.
    pub fn persist_repository(&self) -> Result<(), PersistingError> {
        self.repo.persist_on_disk(&self.config.tag_database)
    }
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
