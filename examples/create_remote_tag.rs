use std::error::Error;

use nextcloud_tag_sync::{load_config, Connection, CreateTag};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let connection = Connection::from_config(&load_config()?);
    connection
        .request(CreateTag::new("fizz".to_owned()))
        .await?;
    Ok(())
}
