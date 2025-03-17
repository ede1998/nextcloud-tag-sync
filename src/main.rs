use std::sync::Arc;

use nextcloud_tag_sync::{Uninitialized, load_config};
use snafu::{Whatever, prelude::*};
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

    ensure_whatever!(
        config.nextcloud_instance.host() == Some(url::Host::Domain("localhost")),
        "use docker nextcloud for test!"
    );

    let mut initialized = Uninitialized::new(config)
        .initialize()
        .await
        .whatever_context("failed to initialize repository")?;
    initialized
        .sync()
        .await
        .whatever_context("failed to sync local to remote")?;
    initialized
        .persist_repository()
        .whatever_context("failed to persist repository")?;

    Ok(())
}
