use std::sync::Arc;

use nextcloud_tag_sync::{
    execute_locally, execute_remotely, load_config, resolve_diffs, ErrorCollection,
    FileSystemLoopError, ListTagsError, LocalFsWalker, RemoteFs, RemoteFsWalker, Repository,
};
use snafu::{prelude::*, FromString, Whatever};
use tokio::task::JoinError;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
#[snafu::report]
async fn main() -> Result<(), Whatever> {
    tracing_subscriber::fmt()
        .with_ansi(atty::is(atty::Stream::Stdout))
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let config = Arc::new(load_config().whatever_context("failed to load config")?);
    info!("Starting with configuration: {config}");
    let walker = RemoteFsWalker::new(&config);
    let remote_repo_task = walker.build_repository();
    let local_repo_task = tokio::task::spawn_blocking({
        let config = config.clone();
        move || LocalFsWalker::new(&config).build_repository()
    });

    let (local, remote, mut remote_fs) =
        convert(futures::join!(local_repo_task, remote_repo_task))?;

    let diff_events = local.diff(remote, config.keep_side_on_conflict);
    let (local_actions, remote_actions) = resolve_diffs(diff_events, config.keep_side_on_conflict);

    println!("{local_actions:#?}");
    println!("{remote_actions:#?}");

    ensure_whatever!(
        config.nextcloud_instance.host() == Some(url::Host::Domain("localhost")),
        "use docker nextcloud for test!"
    );

    execute_remotely(remote_actions, &mut remote_fs, &config).await;
    execute_locally(local_actions, &config);

    Ok(())
}

type LocalFsResult = Result<Repository, FileSystemLoopError>;
type RemoteFsResult = Result<(Repository, RemoteFs), ListTagsError>;

fn convert(
    value: (Result<LocalFsResult, JoinError>, RemoteFsResult),
) -> Result<(Repository, Repository, RemoteFs), Whatever> {
    let (errors, text) = match value {
        (Ok(Ok(l)), Ok(r)) => return Ok((l, r.0, r.1)),
        (Ok(Ok(_)), Err(r)) => (
            ErrorCollection::new(r),
            "failed to initialize remote tag repository",
        ),
        (Ok(Err(l)), Ok(_)) => (
            ErrorCollection::new(l),
            "failed to initialize local tag repository",
        ),
        (Err(e), Ok(_)) => (
            ErrorCollection::new(e),
            "failed to initialize local tag repository",
        ),
        (Ok(Err(l)), Err(r)) => ((l, r).into(), "failed to initialize both tag repositories"),
        (Err(e), Err(r)) => ((e, r).into(), "failed to initialize both tag repositories"),
    };
    Err(Whatever::with_source(errors.into(), text.to_owned()))
}
