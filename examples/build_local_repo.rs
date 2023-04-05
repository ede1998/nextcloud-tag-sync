use nextcloud_tag_sync::{LocalFsWalker, PrefixMapping};
use snafu::Whatever;

#[snafu::report]
fn main() -> Result<(), Whatever> {
    let prefixes = vec![
        PrefixMapping::new("/home/erik/Pictures".into(), "irrelevant here".into()),
        PrefixMapping::new("/home/erik/Documents".into(), "irrelevant here".into()),
    ];

    let repo = LocalFsWalker::new(&prefixes).build_repository()?;
    println!("{repo:?}");

    Ok(())
}
