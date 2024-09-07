use std::error::Error;

use nextcloud_tag_sync::{
    load_config, Connection, FileId, ListFilesWithTag, TagFile, TagId, UntagFile,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let connection = Connection::from_config(&load_config()?);
    // file: /Documents/studium/master/Readme.md
    let file_id = FileId::from(1_978_666);
    // tag: dummy
    let tag_id = TagId::from(739);

    let files = connection.request(ListFilesWithTag::new(tag_id)).await?;
    println!("Tagged files: {files:#?}");

    println!("Now tagging file.");

    connection.request(TagFile::new(tag_id, file_id)).await?;

    let files = connection.request(ListFilesWithTag::new(tag_id)).await?;
    println!("Tagged files: {files:#?}");

    println!("Now un-tagging file.");

    connection.request(UntagFile::new(tag_id, file_id)).await?;

    let files = connection.request(ListFilesWithTag::new(tag_id)).await?;
    println!("Tagged files: {files:#?}");

    Ok(())
}
