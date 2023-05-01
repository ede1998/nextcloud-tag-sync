use std::sync::Arc;

use futures::FutureExt;
use snafu::{ResultExt, Snafu};
use tokio::task::JoinError;

use crate::{
    execute_locally, execute_remotely, resolve_diffs, Config, FileSystemLoopError, ListTagsError,
    LocalFsWalker, RemoteFs, RemoteFsWalker, Repository,
};

pub struct Uninitialized {
    pub config: Arc<Config>,
}

impl Uninitialized {
    async fn create_from_local_remote_diff(&self) -> Result<Initialized, InitError> {
        let walker = RemoteFsWalker::new(&self.config);
        let remote_repo_task = walker.build_repository();
        let local_repo_task = tokio::task::spawn_blocking({
            let config = self.config.clone();
            move || LocalFsWalker::new(&config).build_repository()
        })
        .map(|res| match res {
            Ok(Ok(o)) => Ok(o),
            Ok(Err(e)) => Err(e).context(FilesystemLoopSnafu),
            Err(e) => Err(e).context(JoinSnafu),
        });

        let (local, (remote, mut remote_fs)) =
            match futures::join!(local_repo_task, remote_repo_task) {
                (Ok(l), Ok(r)) => (l, r),
                (Ok(_), Err(e)) => Err(e).context(RemoteSnafu)?,
                (Err(e), Ok(_)) => Err(e).context(LocalSnafu)?,
                (Err(l), Err(r)) => BothSnafu {
                    source_local: l,
                    source_remote: r,
                }
                .fail()?,
            };

        let mut diff_events = local.diff(remote, self.config.keep_side_on_conflict);
        let (local_actions, remote_actions) =
            resolve_diffs(&mut diff_events, self.config.keep_side_on_conflict);

        println!("{local_actions:#?}");
        println!("{remote_actions:#?}");

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
    pub async fn sync_local_to_remote(&mut self) {}

    pub async fn sync_remote_to_local(&mut self) {}
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
