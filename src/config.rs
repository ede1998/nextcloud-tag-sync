use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use url::Url;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::{take_last_n_chars, tag_repository::Side, PrefixMapping};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub max_concurrent_requests: usize,
    pub keep_side_on_conflict: Side,
    pub prefixes: Vec<PrefixMapping>,
    pub nextcloud_instance: Url,
    pub user: String,
    pub token: String,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("max_concurrent_requests", &self.max_concurrent_requests)
            .field("keep_side_on_conflict", &self.keep_side_on_conflict)
            .field("prefixes", &self.prefixes)
            .field("nextcloud_instance", &self.nextcloud_instance)
            .field("user", &self.user)
            .field("token", &"EXPUNGED")
            .finish()
    }
}

impl std::fmt::Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "Configuration:")?;
        writeln!(
            f,
            "Maximum concurrent requests: {}",
            self.max_concurrent_requests
        )?;
        writeln!(
            f,
            "Keep these tags if tags mismatch: {:?}",
            self.keep_side_on_conflict
        )?;
        writeln!(f, "Nextcloud instance: {}", self.nextcloud_instance)?;
        writeln!(f, "Nextcloud user: {}", self.user)?;
        writeln!(
            f,
            "Nextcloud token: ...{}",
            take_last_n_chars(&self.token, 3)
        )?;
        writeln!(f, "Mapped prefixes:")?;
        for prefix in &self.prefixes {
            writeln!(f, "Local:  {}", prefix.local().display())?;
            writeln!(f, "Remote: {}", prefix.remote().display())?;
            writeln!(f)?;
        }
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 10,
            prefixes: Default::default(),
            keep_side_on_conflict: Side::Both,
            nextcloud_instance: "https://missing_nextcloud_instance".try_into().expect("failed to create default url"),
            user: "missing_username".to_owned(),
            token: "missing_token".to_owned(),
        }
    }
}

pub fn load_config() -> Result<Config, ConfigError> {
    Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::file("config.toml"))
        .merge(Env::prefixed("APP_"))
        .extract()
        .context(FigmentSnafu)
}

#[derive(Debug, Snafu)]
pub enum ConfigError {
    #[snafu(display("Failed to set configuration: {source}"))]
    Figment { source: figment::Error },
    #[snafu(display("Configuration already loaded",))]
    AlreadyInitialized,
}
