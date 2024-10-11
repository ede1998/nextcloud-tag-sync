mod common;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};
use tempfile::TempDir;
use test_log::test;

use common::{Nextcloud, Result};
use data_basic::*;
use nextcloud_tag_sync::{
    Config, FileLocation, PrefixMapping, Repository, Side, Tags, Uninitialized,
};
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

    pub fn merged<const N: usize>(tags: [&Tags; N]) -> Option<Tags> {
        let mut first = (*tags.first()?).clone();
        for tag in &tags[1..] {
            first.insert_all(tag);
        }
        Some(first)
    }
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

    pub async fn tag_both(&mut self, file: &str, new_tag: &str) -> Result {
        self.tag_local(file, new_tag)?;
        self.tag_remote(file, new_tag).await?;
        Ok(())
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

    pub fn untag_local(&self, file: impl AsRef<Path>, tag: &str) -> Result {
        let file = self.local_dir(0).join(file.as_ref());
        let tag = tag.parse()?;
        let tag_property = self.config().local_tag_property_name;

        let tags = xattr::get(&file, &tag_property)?.unwrap_or_default();
        let mut tags: Tags = String::from_utf8(tags)?.parse()?;

        tags.remove_one(&tag);
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

    pub async fn untag_remote(&mut self, file: &str, tag: &str) -> Result {
        let file = format!("{}/{file}", self.remote_dir(0));
        let tag = tag.parse()?;
        self.container.untag(&file, &tag).await?;
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

    pub async fn with_prefix(mut self, local: &Path, remote: impl Into<PathBuf>) -> Self {
        async fn copy_recursive(source_directory: &Path, target_directory: &Path) -> Result<()> {
            for entry in WalkDir::new(source_directory) {
                let entry = entry?;
                let meta = entry.metadata()?;
                let source = entry.path();
                let target = target_directory.join(source);
                if meta.is_dir() {
                    tokio::fs::create_dir_all(target).await?;
                } else if meta.is_file() {
                    tokio::fs::copy(source, target).await?;
                }
            }
            Ok(())
        }

        copy_recursive(local, self.temp_dir.path())
            .await
            .expect("Failed to recursively copy files");

        tracing::info!(
            "Copied local directory to temporary location {:?}",
            self.temp_dir
        );

        let local_test_dir = self.temp_dir.path().join(local);
        let mapping = PrefixMapping::new(local_test_dir, remote.into()).expect("invalid mapping");

        self.container
            .upload(path_to_str(mapping.remote()), mapping.local())
            .await
            .expect("Failed to upload files to container");

        self.prefixes.push(mapping);

        self
    }

    pub fn with_db(self, db_json: &str) -> Result<Self> {
        let db = db_json.replace(
            "/tmp/path/to/local/files",
            &self.temp_dir.path().to_string_lossy(),
        );
        std::fs::write(self.config().tag_database, db)?;
        Ok(self)
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
            tag_database: self.temp_dir.path().join("db.json"),
            ..Default::default()
        }
    }

    pub fn arc_config(&self) -> Arc<Config> {
        Arc::new(self.config())
    }

    pub fn assert_snapshot(&self, name: &'static str, repo: &Repository) {
        insta::assert_yaml_snapshot!(name, repo, {
            ".prefixes[].local" => "/tmp/path/to/local/files"
        });
    }

    pub fn assert_db_snapshot(&self, name: &'static str) {
        let temp_dir = self
            .temp_dir
            .path()
            .to_str()
            .expect("non-UTF8 temporary path");
        let db = std::fs::read_to_string(&self.config().tag_database)
            .expect("failed to read tag database")
            .replace(temp_dir, "/tmp/path/to/local/files");
        insta::assert_snapshot!(name, db);
    }

    pub async fn assert_tags(
        &mut self,
        check_location: FileLocation,
        expected: &[(&str, Option<Tags>)],
    ) -> Result {
        for (file, expected_tags) in expected {
            let actual_tags = match check_location {
                FileLocation::Local => self.list_tags_local(file)?,
                FileLocation::Remote => self.list_tags_remote(file).await?,
            };

            let location = match check_location {
                FileLocation::Local => "local",
                FileLocation::Remote => "remote",
            };
            match expected_tags {
                Some(expected_tags) => assert_eq!(&actual_tags, expected_tags, "Wrong tags on {location} file {file}"),
                None => assert!(actual_tags.is_empty(), "Unexpectedly found tags on {location} file {file}"),
            }
        }
        Ok(())
    }
}

#[test(tokio::test)]
async fn sync_tags_basic() -> Result {
    let mut env = TestEnv::new()
        .await
        .with_prefix(&LOCAL_DIR, REMOTE_DIR)
        .await;
    env.tag_local(foo::IGNORE_TXT, tag::SPACE)?;

    env.container
        .sync_tags(REMOTE_DIR, env.prefixes[0].local())
        .await?;

    let tags = env.list_tags_remote(foo::IGNORE_TXT).await?;
    assert_eq!(tags, *tag::SPACE_TAG);
    Ok(())
}

#[test(tokio::test)]
async fn run_initial_sync_to_remote() -> Result {
    let mut env = TestEnv::new()
        .await
        .with_prefix(&LOCAL_DIR, REMOTE_DIR)
        .await;
    env.tag_local(foo::IGNORE_TXT, tag::YELLOW)?;
    env.tag_local(dummy::PLEASE_JPG, tag::SPACE)?;

    let initialized = Uninitialized::new(env.arc_config()).initialize().await?;

    env.assert_snapshot("run_initial_sync_to_remote", initialized.repository());
    let expected = [
        (foo::IGNORE_TXT, Some(tag::YELLOW_TAG.clone())),
        (dummy::PLEASE_JPG, Some(tag::SPACE_TAG.clone())),
        (dummy::ERR_PDF, None),
    ];
    env.assert_tags(FileLocation::Remote, &expected).await?;

    Ok(())
}

#[test(tokio::test)]
async fn run_initial_sync_to_local() -> Result {
    let mut env = TestEnv::new()
        .await
        .with_prefix(&LOCAL_DIR, REMOTE_DIR)
        .await;
    env.tag_remote(foo::IGNORE_TXT, tag::YELLOW).await?;
    env.tag_remote(bar::baz::DRAT_PDF, tag::RED).await?;

    let initialized = Uninitialized::new(env.arc_config()).initialize().await?;

    env.assert_snapshot("run_initial_sync_to_local", initialized.repository());
    let expected = [
        (foo::IGNORE_TXT, Some(tag::YELLOW_TAG.clone())),
        (bar::baz::DRAT_PDF, Some(tag::RED_TAG.clone())),
        (dummy::ERR_PDF, None),
    ];
    env.assert_tags(FileLocation::Local, &expected).await?;

    Ok(())
}

#[test(tokio::test)]
async fn run_initial_sync_bidirectional() -> Result {
    let mut env = TestEnv::new()
        .await
        .with_prefix(&LOCAL_DIR, REMOTE_DIR)
        .await;
    env.tag_both(foo::IGNORE_TXT, tag::YELLOW).await?;
    env.tag_local(dummy::PLEASE_JPG, tag::SPACE)?;
    env.tag_remote(bar::baz::DRAT_PDF, tag::RED).await?;
    env.tag_local(bar::OK_PDF, tag::SPACE)?;
    env.tag_remote(bar::OK_PDF, tag::RED).await?;

    let initialized = Uninitialized::new(env.arc_config()).initialize().await?;

    env.assert_snapshot("run_initial_sync_bidirectional", initialized.repository());
    let expected = [
        (foo::IGNORE_TXT, Some(tag::YELLOW_TAG.clone())),
        (dummy::PLEASE_JPG, Some(tag::SPACE_TAG.clone())),
        (bar::baz::DRAT_PDF, Some(tag::RED_TAG.clone())),
        (bar::OK_PDF, tag::merged([&tag::SPACE_TAG, &tag::RED_TAG])),
        (dummy::ERR_PDF, None),
    ];
    env.assert_tags(FileLocation::Remote, &expected).await?;
    env.assert_tags(FileLocation::Local, &expected).await?;

    initialized.persist_repository()?;

    env.assert_db_snapshot("run_initial_sync_bidirectional_db.json");

    Ok(())
}

#[test(tokio::test)]
async fn ignore_tagged_directory() -> Result {
    let mut env = TestEnv::new()
        .await
        .with_prefix(&LOCAL_DIR, REMOTE_DIR)
        .await;
    env.tag_remote(foo::DIR, tag::YELLOW).await?;
    env.tag_remote(foo::IGNORE_TXT, tag::YELLOW).await?;

    let _ = Uninitialized::new(env.arc_config()).initialize().await?;

    let tags_foo = env.list_tags_local(foo::DIR)?;
    assert!(tags_foo.is_empty());

    Ok(())
}

#[test(tokio::test)]
async fn run_follow_up_sync_bidirectional() -> Result {
    let mut env = TestEnv::new()
        .await
        .with_prefix(&LOCAL_DIR, REMOTE_DIR)
        .await
        .with_db(
            r#"
            {
              "prefixes": [
                {
                  "local": "/tmp/path/to/local/files/tests/data_basic",
                  "remote": "/remote.php/dav/files/tester/test_folder"
                }
              ],
              "files": {
                "0:bar/baz/drat.pdf": [
                  "red"
                ],
                "0:bar/ok.pdf": [
                  "more-tags please",
                  "red"
                ],
                "0:dummy/please.jpg": [
                  "more-tags please"
                ],
                "0:foo/ignore.txt": [
                  "yellow"
                ]
              }
            }"#,
        )?;

    env.tag_both(foo::IGNORE_TXT, tag::YELLOW).await?;
    env.tag_both(dummy::PLEASE_JPG, tag::SPACE).await?;
    env.tag_both(bar::baz::DRAT_PDF, tag::RED).await?;
    env.tag_both(bar::OK_PDF, tag::SPACE).await?;
    env.tag_both(bar::OK_PDF, tag::RED).await?;

    env.tag_local(foo::IGNORE_TXT, tag::RED)?;
    env.tag_remote(dummy::PLEASE_JPG, tag::YELLOW).await?;
    env.untag_local(bar::OK_PDF, tag::RED)?;
    env.untag_remote(bar::OK_PDF, tag::SPACE).await?;
    env.untag_local(bar::baz::DRAT_PDF, tag::RED)?;
    env.untag_remote(bar::baz::DRAT_PDF, tag::RED).await?;

    let mut initialized = Uninitialized::new(env.arc_config()).initialize().await?;
    initialized.sync_local_to_remote().await?;
    initialized.sync_remote_to_local().await?;

    env.assert_snapshot("run_followup_sync_bidirectional", initialized.repository());
    let expected = [
        (
            foo::IGNORE_TXT,
            tag::merged([&tag::YELLOW_TAG, &tag::RED_TAG]),
        ),
        (
            dummy::PLEASE_JPG,
            tag::merged([&tag::SPACE_TAG, &tag::YELLOW_TAG]),
        ),
        (bar::baz::DRAT_PDF, None),
        (bar::OK_PDF, None),
        (dummy::ERR_PDF, None),
    ];
    env.assert_tags(FileLocation::Remote, &expected).await?;
    env.assert_tags(FileLocation::Local, &expected).await?;

    Ok(())
}
