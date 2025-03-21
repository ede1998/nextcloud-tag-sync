use bimap::BiHashMap;
use create_dir::CreateDirectory;
use get_file_tags::GetFileTags;
use nextcloud_tag_sync::{
    Config, Connection, CreateTag, FileId, Tag, TagFile, TagMap, Tags, UntagFile, get_tags_of_file,
};
use testcontainers::{ContainerAsync, Image, core::WaitFor, runners::AsyncRunner as _};
use upload_file::UploadFile;
use url::Url;
use walkdir::WalkDir;

pub type Result<T = (), E = Box<dyn std::error::Error + 'static>> = std::result::Result<T, E>;

mod create_dir;
mod get_file_tags;
mod upload_file;

pub struct NextcloudImage;

impl Image for NextcloudImage {
    fn name(&self) -> &str {
        "nextcloud"
    }

    fn tag(&self) -> &str {
        "29.0.6"
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stderr(
            "Command line: 'apache2 -D FOREGROUND'",
        )]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<
        Item = (
            impl Into<std::borrow::Cow<'_, str>>,
            impl Into<std::borrow::Cow<'_, str>>,
        ),
    > {
        [
            ("SQLITE_DATABASE", ""),
            ("NEXTCLOUD_ADMIN_USER", Nextcloud::ADMIN_USER),
            ("NEXTCLOUD_ADMIN_PASSWORD", Nextcloud::ADMIN_PASSWORD),
        ]
    }
}

pub struct Nextcloud {
    #[allow(dead_code, reason = "Container would be stopped on drop")]
    pub container: ContainerAsync<NextcloudImage>,
    connection: Connection,
    tags: TagMap,
    files: BiHashMap<FileId, String>,
}

impl Nextcloud {
    pub const ADMIN_USER: &'static str = "tester";
    pub const ADMIN_PASSWORD: &'static str = "password";

    pub async fn start() -> Result<Self> {
        let container = NextcloudImage.start().await?;
        let url = url(&container).await?;
        println!("Container started at {url}");
        Ok(Self {
            container,
            connection: Connection::from_config(&Config {
                nextcloud_instance: url,
                user: Self::ADMIN_USER.to_owned(),
                token: Self::ADMIN_PASSWORD.to_owned(),
                ..Default::default()
            }),
            tags: TagMap::default(),
            files: Default::default(),
        })
    }

    pub async fn url(&self) -> Result<Url> {
        url(&self.container).await
    }

    pub async fn upload(&mut self, nc_base_folder: &str, source: &std::path::Path) -> Result {
        // First create folder structure for upload
        for segments in GrowingSegments::new(nc_base_folder, '/').skip(4) {
            self.connection
                .request(CreateDirectory::new(segments))
                .await?;
        }

        let directories = WalkDir::new(source)
            .min_depth(1)
            .into_iter()
            .filter_entry(|p| p.file_type().is_dir());
        for entry in directories {
            let entry = entry?;
            let path = entry.path().strip_prefix(source)?;
            let full_path = format!("{nc_base_folder}/{}", path.display());
            let file_id = self
                .connection
                .request(CreateDirectory::new(&full_path))
                .await?;
            self.files.insert(file_id, full_path);
        }

        // Now upload all files
        let files = WalkDir::new(source).min_depth(1);
        for entry in files {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().strip_prefix(source)?;
            let full_path = format!("{nc_base_folder}/{}", path.display());
            let file_id = self
                .connection
                .request(UploadFile::new(
                    &full_path,
                    tokio::fs::read(entry.path()).await?,
                ))
                .await?;
            self.files.insert(file_id, full_path);
        }

        Ok(())
    }

    pub async fn sync_tags(&mut self, nc_base_folder: &str, source: &std::path::Path) -> Result {
        let files = WalkDir::new(source).min_depth(1);
        for entry in files {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let tags = get_tags_of_file(entry.path(), &Config::default().local_tag_property_name)?;
            let path = entry.path().strip_prefix(source)?;
            let full_path = format!("{nc_base_folder}/{}", path.display());
            for tag in tags {
                self.tag(&full_path, &tag).await?;
            }
        }

        Ok(())
    }

    pub async fn tag(&mut self, file_path: &str, tag: &Tag) -> Result {
        let file_id = *self
            .files
            .get_by_right(file_path)
            .ok_or_else(|| format!("File {file_path} not uploaded"))?;
        let tag_id = match self.tags.get_by_right(tag) {
            Some(tag_id) => *tag_id,
            None => {
                let tag_id = self.connection.request(CreateTag::new(tag.clone())).await?;
                self.tags.insert(tag_id, tag.clone());
                tag_id
            }
        };

        self.connection
            .request(TagFile::new(tag_id, file_id))
            .await?;

        Ok(())
    }

    pub async fn untag(&mut self, file_path: &str, tag: &Tag) -> Result {
        let file_id = *self
            .files
            .get_by_right(file_path)
            .ok_or_else(|| format!("File {file_path} not uploaded"))?;
        let tag_id = match self.tags.get_by_right(tag) {
            Some(tag_id) => *tag_id,
            None => {
                let tag_id = self.connection.request(CreateTag::new(tag.clone())).await?;
                self.tags.insert(tag_id, tag.clone());
                tag_id
            }
        };

        self.connection
            .request(UntagFile::new(tag_id, file_id))
            .await?;

        Ok(())
    }

    pub async fn file_tags(&mut self, file_path: &str) -> Result<Tags> {
        let file_id = *self
            .files
            .get_by_right(file_path)
            .ok_or_else(|| format!("File {file_path} not uploaded"))?;
        Ok(self.connection.request(GetFileTags(file_id)).await?)
    }
}

async fn url(container: &ContainerAsync<NextcloudImage>) -> Result<Url> {
    let host = container.get_host().await?;
    let host_port = container.get_host_port_ipv4(80).await?;
    Ok(Url::parse(&format!("http://{host}:{host_port}"))?)
}

struct GrowingSegments<'a> {
    input: &'a str,
    output: &'a str,
    separator: char,
}

impl<'a> GrowingSegments<'a> {
    pub fn new(input: &'a str, separator: char) -> Self {
        Self {
            input,
            output: if input.starts_with(separator) {
                &input[..separator.len_utf8()]
            } else {
                ""
            },
            separator,
        }
    }
}

impl<'a> Iterator for GrowingSegments<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.input == self.output {
            return None;
        }
        let returned_len = self.output.len();
        let remainder = &self.input[returned_len..];
        let next_separator = remainder.find(self.separator).map(|pos| pos + returned_len);
        match next_separator {
            Some(next_separator) => {
                let result = &self.input[..next_separator];
                self.output = &self.input[..(next_separator + self.separator.len_utf8())];
                Some(result)
            }
            None => {
                self.output = self.input;
                Some(self.output)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GrowingSegments;

    #[test]
    fn growing_segments() {
        let mut iter = GrowingSegments::new("/remote.php/dav/files/tester/asdf", '/');
        assert_eq!(Some("/remote.php"), iter.next());
        assert_eq!(Some("/remote.php/dav"), iter.next());
        assert_eq!(Some("/remote.php/dav/files"), iter.next());
        assert_eq!(Some("/remote.php/dav/files/tester"), iter.next());
        assert_eq!(Some("/remote.php/dav/files/tester/asdf"), iter.next());
    }

    #[test]
    fn single_word() {
        let mut iter = GrowingSegments::new("single_word", '/');
        assert_eq!(Some("single_word"), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn no_leading_separator() {
        let mut iter = GrowingSegments::new("split/in/middle", '/');
        assert_eq!(Some("split"), iter.next());
        assert_eq!(Some("split/in"), iter.next());
        assert_eq!(Some("split/in/middle"), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn multi_separator() {
        let mut iter = GrowingSegments::new("/split//again", '/');
        assert_eq!(Some("/split"), iter.next());
        assert_eq!(Some("/split/"), iter.next());
        assert_eq!(Some("/split//again"), iter.next());
        assert_eq!(None, iter.next());
    }
}
