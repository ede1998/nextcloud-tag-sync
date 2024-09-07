use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{tag_repository::Side, take_last_n_chars, PrefixMapping};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub max_concurrent_requests: usize,
    pub keep_side_on_conflict: Side,
    pub prefixes: Vec<PrefixMapping>,
    pub nextcloud_instance: Url,
    pub user: String,
    pub token: String,
    pub local_tag_property_name: String,
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
            prefixes: Vec::default(),
            keep_side_on_conflict: Side::Both,
            nextcloud_instance: "https://missing_nextcloud_instance"
                .try_into()
                .expect("failed to create default url"),
            user: "missing_username".to_owned(),
            token: "missing_token".to_owned(),
            local_tag_property_name: "user.xdg.tags".to_owned(),
        }
    }
}

/// Load the configuration from environment variables, config.toml or compile time defaults.
///
/// # Errors
///
/// This function will return an error if configuration loading encounters invalid values or
/// fails to load the configuration files.
pub fn load_config() -> Result<Config, figment::Error> {
    Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::file("config.toml"))
        .merge(Env::prefixed("APP_"))
        .extract()
}
