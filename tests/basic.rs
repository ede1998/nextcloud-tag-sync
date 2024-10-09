mod common;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};
use tempfile::TempDir;
use test_log::test;

use common::{Nextcloud, Result};
use data_basic::*;
use nextcloud_tag_sync::{Config, PrefixMapping, Side, Tags, Uninitialized};
use url::Url;
use walkdir::WalkDir;

static LOCAL_DIR: LazyLock<PathBuf> = LazyLock::new(|| "tests/data_basic".into());
const REMOTE_DIR: &str = "/remote.php/dav/files/tester/test_folder";

mod tag {
    use std::sync::LazyLock;

    use nextcloud_tag_sync::Tags;

    pub const YELLOW: &str = "yellow";
    pub static YELLOW_TAG: LazyLock<Tags> = LazyLock::new(|| YELLOW.parse().unwrap());

    pub const RED: &str = "red";
    pub static RED_TAG: LazyLock<Tags> = LazyLock::new(|| RED.parse().unwrap());

    pub const SPACE: &str = "more-tags please";
    pub static SPACE_TAG: LazyLock<Tags> = LazyLock::new(|| SPACE.parse().unwrap());
}

#[allow(
    dead_code,
    reason = "Exact replica of the example data directory structure"
)]
mod data_basic {
    pub mod bar {
        pub const DIR: &str = "bar";
        pub const OK_PDF: &str = "bar/ok.pdf";
        pub mod baz {
            pub const DIR: &str = "bar/baz";
            pub const DRAT_PDF: &str = "bar/baz/drat.pdf";
            pub const RANDOM_TXT: &str = "bar/baz/random.txt";
        }
    }

    pub mod dummy {
        pub const DIR: &str = "dummy";
        pub const ERR_PDF: &str = "dummy/err.pdf";
        pub const PLEASE_JPG: &str = "dummy/please.jpg";
    }

    pub mod foo {
        pub const DIR: &str = "foo";
        pub const IGNORE_TXT: &str = "foo/ignore.txt";
    }
}

fn path_to_str(p: &Path) -> &str {
    p.as_os_str().to_str().expect("non-UTF8 path")
}

struct TestEnv {
    pub keep_side_on_conflict: Side,
    pub prefixes: Vec<PrefixMapping>,
    pub nextcloud_instance: Url,
    pub user: String,
    pub token: String,
    pub temp_dir: TempDir,
    pub container: Nextcloud,
}

impl TestEnv {
    pub async fn new() -> Self {
        let container = Nextcloud::start()
            .await
            .expect("Failed to start Nextcloud container");
        Self {
            keep_side_on_conflict: Side::Both,
            prefixes: Vec::new(),
            nextcloud_instance: container.url().await.expect("Failed to read Nextcloud URL"),
            user: Nextcloud::ADMIN_USER.to_owned(),
            token: Nextcloud::ADMIN_PASSWORD.to_owned(),
            temp_dir: tempfile::tempdir().expect("Failed to create temp dir"),
            container,
        }
    }

    pub fn tag_local(&self, file: impl AsRef<Path>, new_tag: &str) -> Result {
        let file = self.local_dir(0).join(file.as_ref());
        let new_tag = new_tag.parse()?;
        let tag_property = self.config().local_tag_property_name;

        let tags = xattr::get(&file, &tag_property)?.unwrap_or_default();
        let mut tags: Tags = String::from_utf8(tags)?.parse()?;

        tags.insert_one(new_tag);
        let stringified_tags = tags.to_string();
        xattr::set(&file, &tag_property, stringified_tags.as_bytes())?;
        Ok(())
    }

    pub async fn tag_remote(&mut self, file: &str, new_tag: &str) -> Result {
        let file = format!("{}/{file}", self.remote_dir(0));
        let new_tag = new_tag.parse()?;
        self.container.tag(&file, &new_tag).await?;
        Ok(())
    }

    pub fn list_tags_local(&self, file: impl AsRef<Path>) -> Result<Tags> {
        let file = self.local_dir(0).join(file.as_ref());
        let tag_property = self.config().local_tag_property_name;

        let tags = xattr::get(&file, &tag_property)?.unwrap_or_default();
        Ok(String::from_utf8(tags)?.parse()?)
    }

    pub async fn list_tags_remote(&mut self, file: &str) -> Result<Tags> {
        let file = format!("{}/{file}", self.remote_dir(0));
        self.container.file_tags(&file).await
    }

    pub async fn with_prefix(
        mut self,
        local: impl Into<PathBuf>,
        remote: impl Into<PathBuf>,
    ) -> Self {
        let mapping = PrefixMapping::new(local.into(), remote.into()).expect("invalid mapping");

        async fn copy_recursive(temp_dir: &TempDir, mapping: &PrefixMapping) -> Result<()> {
            for entry in WalkDir::new(mapping.local()) {
                let entry = entry?;
                let meta = entry.metadata()?;
                let source = entry.path();
                let target = temp_dir.path().join(source);
                if meta.is_dir() {
                    tokio::fs::create_dir_all(target).await?;
                } else if meta.is_file() {
                    tokio::fs::copy(source, target).await?;
                }
            }
            Ok(())
        }

        copy_recursive(&self.temp_dir, &mapping)
            .await
            .expect("Failed to recursively copy files");

        self.container
            .upload(path_to_str(mapping.remote()), mapping.local())
            .await
            .expect("Failed to upload files to container");

        self.prefixes.push(mapping);

        self
    }

    pub fn local_dir(&self, index: usize) -> &Path {
        self.prefixes[index].local()
    }

    pub fn remote_dir(&self, index: usize) -> &str {
        path_to_str(self.prefixes[index].remote())
    }

    pub fn config(&self) -> Config {
        Config {
            keep_side_on_conflict: self.keep_side_on_conflict,
            prefixes: self.prefixes.clone(),
            nextcloud_instance: self.nextcloud_instance.clone(),
            user: self.user.clone(),
            token: self.token.clone(),
            max_concurrent_requests: 100,
            ..Default::default()
        }
    }

    pub fn arc_config(&self) -> Arc<Config> {
        Arc::new(self.config())
    }
}

#[test(tokio::test)]
async fn sync_tags_basic() -> Result {
    let mut container = Nextcloud::start().await?;
    container.upload(REMOTE_DIR, &LOCAL_DIR).await?;
    container.sync_tags(REMOTE_DIR, &LOCAL_DIR).await?;
    let tags = container
        .file_tags(&format!("{REMOTE_DIR}/dummy/please.jpg"))
        .await?;
    assert_eq!(tags, "more-tags please".parse()?);
    Ok(())
}

#[test(tokio::test)]
async fn run_initial_sync_to_remote() -> Result {
    let mut env = TestEnv::new()
        .await
        .with_prefix(&*LOCAL_DIR, REMOTE_DIR)
        .await;
    env.tag_local(foo::IGNORE_TXT, tag::YELLOW)?;
    env.tag_local(dummy::PLEASE_JPG, tag::SPACE)?;

    let initialized = Uninitialized::new(env.arc_config()).initialize().await?;

    insta::assert_yaml_snapshot!("run_initial_sync_to_remote", initialized.repository());
    let tags_ignore_txt = env.list_tags_remote(foo::IGNORE_TXT).await?;
    assert_eq!(tags_ignore_txt, *tag::YELLOW_TAG);
    let tags_please_jpg = env.list_tags_remote(dummy::PLEASE_JPG).await?;
    assert_eq!(tags_please_jpg, *tag::SPACE_TAG);
    let tags_err_pdf = env.list_tags_remote(dummy::ERR_PDF).await?;
    assert!(tags_err_pdf.is_empty());

    Ok(())
}

#[test(tokio::test)]
async fn run_initial_sync_to_local() -> Result {
    let mut env = TestEnv::new()
        .await
        .with_prefix(&*LOCAL_DIR, REMOTE_DIR)
        .await;
    env.tag_remote(foo::IGNORE_TXT, tag::YELLOW).await?;
    env.tag_remote(bar::baz::DRAT_PDF, tag::RED).await?;

    let initialized = Uninitialized::new(env.arc_config()).initialize().await?;

    insta::assert_yaml_snapshot!("run_initial_sync_to_local", initialized.repository());
    let tags_ignore_txt = env.list_tags_local(foo::IGNORE_TXT)?;
    assert_eq!(tags_ignore_txt, *tag::YELLOW_TAG);
    let tags_drat_pdf = env.list_tags_local(bar::baz::DRAT_PDF)?;
    assert_eq!(tags_drat_pdf, *tag::RED_TAG);
    let tags_err_pdf = env.list_tags_local(dummy::ERR_PDF)?;
    assert!(tags_err_pdf.is_empty());

    Ok(())
}

// sync with pre-existing tags on both sides
// already synced -> detect diff
// test directory tagged in nextcloud
