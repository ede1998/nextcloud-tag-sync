use std::error::Error;

use nextcloud_tag_sync::{Connection, ListFilesWithTag, ListTags, Tag, load_config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let connection = Connection::from_config(&load_config()?);
    let tags = connection.request(ListTags).await?;
    println!("List of all tags:\n{tags:?}");
    let tag_name: Tag = "Alligator".parse()?;
    let tag_id = *tags.get_by_right(&tag_name).unwrap();
    let files = connection.request(ListFilesWithTag::new(tag_id)).await?;
    println!("Files tagged with {tag_name} are: {files:?}");
    Ok(())
}
