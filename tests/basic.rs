mod common;

use std::{path::Path, time::Duration};

use common::Nextcloud;
use tokio::time::sleep;

#[tokio::test]
async fn test_redis() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let container = Nextcloud::start().await?;
    println!("{}", container.url().await?);
    container.upload("squirtle/test_folder", Path::new("manual-testing/test_folder")).await?;
    sleep(Duration::from_secs(60)).await;
    Ok(())
}
