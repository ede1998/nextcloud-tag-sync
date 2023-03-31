use std::error::Error;

use nextcloud_tag_sync::{Connection, ListFilesWithTag, ListTags};

#[tokio::main]
async fn main() {
    async fn m() -> Result<(), Box<dyn Error>> {
        let connection = Connection::default();
        let tags = connection.request(ListTags).await?;
        println!("List of all tags:\n{tags}");
        let tag_name = "Alligator";
        let tag_id = *tags.get_first(tag_name).unwrap();
        let files = connection.request(ListFilesWithTag::new(tag_id)).await?;
        println!("Files tagged with {tag_name} are: {files:?}");
        Ok(())
    }
    match m().await {
        Ok(()) => {}
        Err(e) => println!("{e}"),
    }
}
