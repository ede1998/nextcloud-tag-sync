use std::sync::Arc;

use futures::FutureExt;
use snafu::{ResultExt, Snafu};
use tokio::task::JoinError;

use crate::{
    execute_locally, execute_remotely, resolve_diffs, tag_repository::Side, Config,
    FileSystemLoopError, ListTagsError, LocalFsWalker, RemoteFs, RemoteFsWalker, Repository, CommandsFormatter,
};

pub struct Uninitialized {
    pub config: Arc<Config>,
}

impl Uninitialized {
    async fn create_from_local_remote_diff(&self) -> Result<Initialized, InitError> {
        let remote_repo_task = run_remote_walker(&self.config);
        let local_repo_task = run_local_walker(self.config.clone());

        let (local, (remote, mut remote_fs)) =
            merge_results(futures::join!(local_repo_task, remote_repo_task))?;

        let mut diff_events = local.diff(remote, self.config.keep_side_on_conflict);
        let (local_actions, remote_actions) =
            resolve_diffs(&mut diff_events, self.config.keep_side_on_conflict);

        let cmd_fmt = CommandsFormatter(&local_actions);
        println!("{cmd_fmt}");
        let cmd_fmt = CommandsFormatter(&remote_actions);
        println!("{cmd_fmt}");

        execute_remotely(remote_actions, &mut remote_fs, &self.config).await;
        execute_locally(local_actions, &self.config);

        Ok(Initialized {
            repo: diff_events.finish(),
            remote_fs,
            config: self.config.clone(),
        })
    }

    fn load_from_file(&self) -> Option<Initialized> {
        None
    }

    pub async fn initialize(self) -> Result<Initialized, InitError> {
        match self.load_from_file() {
            Some(o) => Ok(o),
            None => self.create_from_local_remote_diff().await,
        }
    }
}

pub struct Initialized {
    repo: Repository,
    remote_fs: RemoteFs,
    config: Arc<Config>,
}

impl Initialized {
    pub async fn sync_local_to_remote(&mut self) -> Result<(), InitError> {
        let config = self.config.clone();
        let local = run_local_walker(config).await.context(LocalSnafu)?;

        let repo = std::mem::take(&mut self.repo);
        let mut diff_events = repo.diff(local, Side::Right);
        let (_, actions) = resolve_diffs(&mut diff_events, Side::Right);

        let cmd_fmt = CommandsFormatter(&actions);
        println!("{cmd_fmt}");

        execute_remotely(actions, &mut self.remote_fs, &self.config).await;
        self.repo = diff_events.finish();
        Ok(())
    }

    pub async fn sync_remote_to_local(&mut self) -> Result<(), InitError> {
        let (remote, remote_fs) = run_remote_walker(&self.config).await.context(RemoteSnafu)?;

        let repo = std::mem::take(&mut self.repo);
        let mut diff_events = repo.diff(remote, Side::Right);
        let (_, actions) = resolve_diffs(&mut diff_events, Side::Right);

        let cmd_fmt = CommandsFormatter(&actions);
        println!("{cmd_fmt}");

        execute_locally(actions, &self.config);

        self.repo = diff_events.finish();
        self.remote_fs.assimilate(remote_fs);
        Ok(())
    }
}

async fn run_local_walker(config: Arc<Config>) -> Result<Repository, LocalError> {
    tokio::task::spawn_blocking(move || LocalFsWalker::new(&config).build_repository())
        .map(|res| match res {
            Ok(Ok(o)) => Ok(o),
            Ok(Err(e)) => Err(e).context(FilesystemLoopSnafu),
            Err(e) => Err(e).context(JoinSnafu),
        })
        .await
}

async fn run_remote_walker(config: &Config) -> Result<(Repository, RemoteFs), ListTagsError> {
    let walker = RemoteFsWalker::new(config);
    walker.build_repository().await
}

#[allow(clippy::result_large_err)] // only runs once -> no performance issue anyway
fn merge_results<T, U>(
    results: (Result<T, LocalError>, Result<U, ListTagsError>),
) -> Result<(T, U), InitError> {
    match results {
        (Ok(l), Ok(r)) => Ok((l, r)),
        (Ok(_), Err(e)) => Err(e).context(RemoteSnafu)?,
        (Err(e), Ok(_)) => Err(e).context(LocalSnafu)?,
        (Err(l), Err(r)) => BothSnafu {
            source_local: l,
            source_remote: r,
        }
        .fail()?,
    }
}

#[derive(Snafu, Debug)]
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

#[derive(Debug, Snafu)]
pub enum LocalError {
    Join { source: JoinError },
    FilesystemLoop { source: FileSystemLoopError },
}
