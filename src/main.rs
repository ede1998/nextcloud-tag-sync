use std::sync::Arc;

use nextcloud_tag_sync::{
    load_config, Connection, ErrorCollection, FileSystemLoopError, ListTagsError, LocalFsWalker,
    RemoteFsWalker, Repository,
};
use snafu::{prelude::*, FromString, Whatever};
use tokio::task::JoinError;
use tracing::info;

#[tokio::main]
#[snafu::report]
async fn main() -> Result<(), Whatever> {
    tracing_subscriber::fmt::init();
    let config = Arc::new(load_config().whatever_context("failed to load config")?);
    info!("Starting with configuration: {config}");
    let connection = Connection::from_config(&config);
    let walker = RemoteFsWalker::new(connection, &config.prefixes, config.max_concurrent_requests);
    let remote_repo_task = walker.build_repository();
    let local_repo_task = tokio::task::spawn_blocking({
        let config = config.clone();
        move || LocalFsWalker::new(&config.prefixes).build_repository()
    });

    let (local, remote) = convert(futures::join!(local_repo_task, remote_repo_task))?;

    let diff_events = local.diff(remote, config.keep_side_on_conflict);
    for diff_event in diff_events {
        println!("{diff_event:?}");
    }

    Ok(())
}

fn convert(
    value: (
        Result<Result<Repository, FileSystemLoopError>, JoinError>,
        Result<Repository, ListTagsError>,
    ),
) -> Result<(Repository, Repository), Whatever> {
    let (errors, text) = match value {
        (Ok(Ok(l)), Ok(r)) => return Ok((l, r)),
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
