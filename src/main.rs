use std::error::Error;

use requests::{Connection, ListTags};

mod deserializers;
mod map;
mod requests;

#[tokio::main]
async fn main() {
    async fn m() -> Result<(), Box<dyn Error>> {
        let connection = Connection::default();
        let tags = connection.request(ListTags).await?;
        println!("{tags}");
        Ok(())
    }
    match m().await {
        Ok(()) => {}
        Err(e) => println!("{e}"),
    }
}
