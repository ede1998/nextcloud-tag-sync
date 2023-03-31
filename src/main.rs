use std::error::Error;

use requests::{Connection, ListFilesWithTag, ListTags};

mod map;
mod requests;

#[tokio::main]
async fn main1() {
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

use futures::{channel::mpsc::channel, SinkExt, StreamExt};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

#[tokio::main]
async fn main() {
    let path = "/home/erik/Documents";
    futures::executor::block_on(async {
        if let Err(e) = async_watch([path]).await {
            println!("error: {:?}", e)
        }
    });
}

async fn async_watch<P, I>(paths: I) -> notify::Result<()>
where
    P: AsRef<Path>,
    I: IntoIterator<Item = P>,
{
    let (mut tx, mut rx) = channel(100);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            futures::executor::block_on(async {
                if let Err(e) = tx.send(res).await {
                    if e.is_full() {
                        println!("Dropping event because queue is full.");
                    }
                }
            })
        },
        Config::default(),
    )?;

    for path in paths {
        watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;
    }

    while let Some(res) = rx.next().await {
        match res {
            Ok(event) => println!("changed: {:?}", event),
            Err(e) => println!("watch error: {:?}", e),
        }
    }

    Ok(())
}
