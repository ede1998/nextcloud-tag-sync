use reqwest::{Method, RequestBuilder};
use testcontainers::{core::WaitFor, runners::AsyncRunner as _, ContainerAsync, Image};
use walkdir::WalkDir;

pub type Result<T = (), E = Box<dyn std::error::Error + 'static>> = std::result::Result<T, E>;

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
    pub container: ContainerAsync<NextcloudImage>,
    client: reqwest::Client,
}

impl Nextcloud {
    pub const ADMIN_USER: &'static str = "tester";
    pub const ADMIN_PASSWORD: &'static str = "password";

    pub async fn start() -> Result<Self> {
        let container = NextcloudImage.start().await?;
        Ok(Self {
            container,
            client: reqwest::Client::new(),
        })
    }

    pub async fn url(&self) -> Result<String> {
        let host = self.container.get_host().await?;
        let host_port = self.container.get_host_port_ipv4(80).await?;
        Ok(format!("http://{host}:{host_port}"))
    }

    // # delete existing files in test folder
    // curl --silent --user "$NC_USER:$NC_PASSWORD" 'http://'"$NC_HOST":"$NC_PORT"'/remote.php/dav/files/'"$NC_USER/$NC_FOLDER" --request DELETE > /dev/null || true

    pub async fn upload(&self, nc_base_folder: &str, source: &std::path::Path) -> Result {
        let root_file_url = format!(
            "{}/remote.php/dav/files/{}",
            self.url().await?,
            Self::ADMIN_USER
        );
        let base_folder_url = format!("{root_file_url}/{nc_base_folder}");

        // First create folder structure for upload
        async fn mkdir(this: &Nextcloud, dir_url: impl AsRef<str>) -> Result {
            this.request(Method::from_bytes(b"MKCOL")?, dir_url.as_ref())
                .send()
                .await?
                .error_for_status()?;
            Ok(())
        }

        for segments in GrowingSegments::new(nc_base_folder, '/') {
            mkdir(self, format!("{root_file_url}/{segments}")).await?;
        }

        let directories = WalkDir::new(source)
            .min_depth(1)
            .into_iter()
            .filter_entry(|p| p.file_type().is_dir());
        for entry in directories {
            let entry = entry?;
            let path = entry.path().strip_prefix(source)?;
            mkdir(self, format!("{base_folder_url}/{}", path.display())).await?;
        }

        // Now upload all files
        let files = WalkDir::new(source).min_depth(1);
        for entry in files {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let file_contents = tokio::fs::read(entry.path()).await?;
            let path = entry.path().strip_prefix(source)?;
            dbg!(self.request(Method::PUT, format!("{base_folder_url}/{}", path.display()))
                            .body(file_contents)
                            .send()
                            .await?
                            .error_for_status()?);
        }

        Ok(())
    }

    fn request<U: reqwest::IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        self.client
            .request(method, url)
            .basic_auth(Self::ADMIN_USER, Some(Self::ADMIN_PASSWORD))
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
