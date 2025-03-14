use std::error::Error;

use nextcloud_tag_sync::{Connection, CreateTag, load_config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let connection = Connection::from_config(&load_config()?);
    connection
        .request(CreateTag::new("fizz".parse().unwrap()))
        .await?;
    Ok(())
}
