use std::error::Error;

use nextcloud_tag_sync::Tags;

fn main() -> Result<(), Box<dyn Error>> {
    let file_path = "helper-scripts/sample.txt";
    let tag_name = "user.xdg.tags";
    let tag = xattr::get(file_path, tag_name)?.unwrap_or_default();
    let tag = String::from_utf8(tag)?;
    let mut tag: Tags = tag.parse()?;
    tag.insert_one("anotherone".to_owned());
    tag.insert_one("yay".to_owned());
    let stringified_tags = tag.to_string();
    xattr::set(file_path, tag_name, stringified_tags.as_bytes())?;
    Ok(())
}
