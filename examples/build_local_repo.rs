use nextcloud_tag_sync::{load_config, Config, LocalFsWalker, PrefixMapping};
use snafu::{ResultExt, Whatever};

#[snafu::report]
fn main() -> Result<(), Whatever> {
    let config = Config {
        prefixes: vec![
            PrefixMapping::new("/home/erik/Pictures".into(), "irrelevant here".into()),
            PrefixMapping::new("/home/erik/Documents".into(), "irrelevant here".into()),
        ],
        ..load_config().whatever_context("failed to load config")?
    };

    let repo = LocalFsWalker::new(&config).build_repository();
    println!("{repo:?}");

    Ok(())
}
