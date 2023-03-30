use std::error::Error;

use requests::{Connection, ListFilesWithTag, ListTags};

mod deserializers;
mod map;
mod requests;

#[tokio::main]
async fn main() {
    async fn m() -> Result<(), Box<dyn Error>> {
        let connection = Connection::default();
        let tags = connection.request(ListTags).await?;
        println!("List of all tags:\n{tags}");
        let files = connection.request(ListFilesWithTag::new(382)).await?;
        println!("All alligators are here: {files:?}");
        Ok(())
    }
    match m().await {
        Ok(()) => {}
        Err(e) => println!("{e}"),
    }
}
