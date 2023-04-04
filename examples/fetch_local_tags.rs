use std::error::Error;

use walkdir::WalkDir;

fn main() -> Result<(), Box<dyn Error>> {
    // call tokio::task::spawn_blocking for running in async rt
    let path = "/home/erik/Documents";
    let walker = WalkDir::new(path);
    for entry in walker {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        let tag = xattr::get(entry.path(), "user.xdg.tags")?;
        if let Some(Ok(tag)) = tag.map(String::from_utf8) {
            println!("{}: {tag}", entry.path().display());
        }
    }

    Ok(())
}
