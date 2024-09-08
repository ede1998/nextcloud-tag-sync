use create_dir::CreateDirectory;
use nextcloud_tag_sync::{Config, Connection};
use testcontainers::{core::WaitFor, runners::AsyncRunner as _, ContainerAsync, Image};
use upload_file::UploadFile;
use url::Url;
use walkdir::WalkDir;

pub type Result<T = (), E = Box<dyn std::error::Error + 'static>> = std::result::Result<T, E>;

mod create_dir;
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
}

impl Nextcloud {
    pub const ADMIN_USER: &'static str = "tester";
    pub const ADMIN_PASSWORD: &'static str = "password";

    pub async fn start() -> Result<Self> {
        let container = NextcloudImage.start().await?;
        let host = container.get_host().await?;
        let host_port = container.get_host_port_ipv4(80).await?;
        let url = Url::parse(&format!("http://{host}:{host_port}"))?;
        println!("Container started at {}", url.as_str());
        Ok(Self {
            container,
            connection: Connection::from_config(&Config {
                nextcloud_instance: url,
                user: Self::ADMIN_USER.to_owned(),
                token: Self::ADMIN_PASSWORD.to_owned(),
                ..Default::default()
            }),
        })
    }

    pub async fn upload(&self, nc_base_folder: &str, source: &std::path::Path) -> Result {
        // First create folder structure for upload
        for segments in GrowingSegments::new(nc_base_folder, '/') {
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
            self.connection
                .request(CreateDirectory::new(format!(
                    "{nc_base_folder}/{}",
                    path.display()
                )))
                .await?;
        }

        // Now upload all files
        let files = WalkDir::new(source).min_depth(1);
        for entry in files {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().strip_prefix(source)?;
            self.connection
                .request(UploadFile::new(
                    format!("{nc_base_folder}/{}", path.display()),
                    tokio::fs::read(entry.path()).await?,
                ))
                .await?;
        }

        Ok(())
    }
}

struct GrowingSegments<'a> {
    input: &'a str,
    index: usize,
    separator: char,
}

impl<'a> GrowingSegments<'a> {
    pub fn new(input: &'a str, separator: char) -> Self {
        Self {
            input,
            index: 0,
            separator,
        }
    }
}

impl<'a> Iterator for GrowingSegments<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.input.len() == self.index {
            return None;
        }

        // Safety: The index is either 0 and hence always safe, or determined with find
        // in a previous iteration and therefore in bounds and on a UTF-8 boundary.
        let remainder =
            unsafe { self.input.get_unchecked(self.index..) }.trim_start_matches(self.separator);
        self.index = remainder.find(self.separator).unwrap_or(self.input.len());

        // Safety: index was just determined with find and thus in bounds and on UTF-8 boundary.
        Some(unsafe { self.input.get_unchecked(..self.index) })
    }
}
