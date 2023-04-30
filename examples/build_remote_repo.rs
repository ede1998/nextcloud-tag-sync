use std::error::Error;

use nextcloud_tag_sync::{load_config, Config, PrefixMapping, RemoteFsWalker};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    let config = Config {
        prefixes: vec![
            PrefixMapping::new(
                "irrelevant here".into(),
                "/remote.php/dav/files/erik/Pictures".into(),
            ),
            PrefixMapping::new(
                "irrelevant here".into(),
                "/remote.php/dav/files/erik/Documents".into(),
            ),
        ],
        ..load_config()?
    };

    let repo = RemoteFsWalker::new(&config).build_repository().await?.0;

    println!("{repo:?}");
    Ok(())
}
