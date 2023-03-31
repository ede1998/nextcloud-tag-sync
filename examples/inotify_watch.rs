use futures::{channel::mpsc::channel, SinkExt, StreamExt};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

#[tokio::main]
async fn main() {
    let path = ["/home/erik/Documents", "/home/erik/Pictures"];
    futures::executor::block_on(async {
        if let Err(e) = async_watch(path).await {
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
