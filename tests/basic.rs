mod common;

use std::path::Path;

use common::Nextcloud;

#[tokio::test]
async fn test_redis() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let mut container = Nextcloud::start().await?;
    let local_dir = Path::new("manual-testing/test_folder");
    let remote_dir = "squirtle/test_folder";
    container.upload(remote_dir, local_dir).await?;
    container.sync_tags(remote_dir, local_dir).await?;
    let tags = container
        .file_tags("squirtle/test_folder/dummy/please.jpg")
        .await?;
    assert_eq!(tags, "more-tags please".parse()?);
    // sleep(Duration::from_secs(60)).await;
    Ok(())
}

// sync local to remote, no tags remote
// sync remote to local, no tags local
// sync with pre-existing tags on both sides
// already synced -> detect diff
