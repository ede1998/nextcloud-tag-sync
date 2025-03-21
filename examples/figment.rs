use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
struct Config {
    key: String,
    another: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            key: "default".into(),
            another: 100,
        }
    }
}

#[expect(clippy::result_large_err, reason = "only called once")]
fn main() -> figment::error::Result<()> {
    let config: Config = Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::file("App.toml"))
        .merge(Env::prefixed("APP_"))
        .extract()?;

    println!("{config:?}");
    Ok(())
}
