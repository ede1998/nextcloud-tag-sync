use std::error::Error;

use nextcloud_tag_sync::{load_config, Connection, PrefixMapping, RemoteFsWalker};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    let connection = Connection::from_config(&load_config()?);

    let prefixes = vec![
        PrefixMapping::new(
            "irrelevant here".into(),
            "/remote.php/dav/files/erik/Pictures".into(),
        ),
        PrefixMapping::new(
            "irrelevant here".into(),
            "/remote.php/dav/files/erik/Documents".into(),
        ),
    ];

    let repo = RemoteFsWalker::new(connection, &prefixes, 10)
        .build_repository()
        .await?;

    println!("{repo:?}");
    Ok(())
}
