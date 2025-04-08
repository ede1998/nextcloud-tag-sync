use std::path::{Path, PathBuf};

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{PrefixMapping, tag_repository::Side, take_last_n_chars};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub max_concurrent_requests: usize,
    pub keep_side_on_conflict: Side,
    pub prefixes: Vec<PrefixMapping>,
    pub nextcloud_instance: Url,
    pub user: String,
    pub token: String,
    pub local_tag_property_name: String,
    pub tag_database: std::path::PathBuf,
    pub dry_run: bool,
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
            .field("local_tag_property_name", &self.local_tag_property_name)
            .field("tag_database", &self.tag_database)
            .field("dry_run", &self.dry_run)
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
        writeln!(f, "Tag database: {}", self.tag_database.display())?;
        writeln!(f, "Nextcloud instance: {}", self.nextcloud_instance)?;
        writeln!(f, "Nextcloud user: {}", self.user)?;
        writeln!(
            f,
            "Nextcloud token: ...{}",
            take_last_n_chars(&self.token, 3)
        )?;
        writeln!(f, "Dry-Run: {}", self.dry_run)?;
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
            prefixes: Vec::default(),
            keep_side_on_conflict: Side::Both,
            nextcloud_instance: "https://missing_nextcloud_instance"
                .try_into()
                .expect("failed to create default url"),
            user: "missing_username".to_owned(),
            token: "missing_token".to_owned(),
            local_tag_property_name: "user.xdg.tags".to_owned(),
            tag_database: PathBuf::from("nextcloud-tag-sync.db.json"),
            dry_run: true,
        }
    }
}

/// Load the configuration from environment variables, config.toml or compile time defaults.
///
/// # Errors
///
/// This function will return an error if configuration loading encounters invalid values or
/// fails to load the configuration files.
#[expect(clippy::result_large_err, reason = "Only called once")]
pub fn load_config() -> figment::error::Result<Config> {
    const FILE_NAME: &str = "nextcloud-tag-sync.toml";
    let toml = Path::new(FILE_NAME)
        .exists()
        .then(|| Toml::file_exact(FILE_NAME))
        .or_else(|| dirs::config_dir().map(|cfg| Toml::file_exact(cfg.join(FILE_NAME))))
        .unwrap_or_else(|| Toml::file(FILE_NAME));
    Figment::from(Serialized::defaults(Config::default()))
        .merge(toml)
        .merge(Env::prefixed("NCTS_"))
        .extract()
}
