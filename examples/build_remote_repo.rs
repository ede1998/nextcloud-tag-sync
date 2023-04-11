use std::error::Error;

use nextcloud_tag_sync::{Connection, PrefixMapping, RemoteFsWalker, load_config};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    async fn m() -> Result<(), Box<dyn Error>> {
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
    match m().await {
        Ok(()) => {}
        Err(e) => println!("{e}"),
    }
}
