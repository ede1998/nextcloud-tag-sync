use std::error::Error;

use nextcloud_tag_sync::{load_config, Connection, FileId, TagFile, TagId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let connection = Connection::from_config(&load_config()?);
    // file: /Documents/studium/master/Readme.md
    let file_id = FileId::from(1978666);
    // tag: dummy
    let tag_id = TagId::from(739);
    connection.request(TagFile::new(tag_id, file_id)).await?;
    Ok(())
}
