use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    load_config().unwrap_or_else(|err| {
        log::error!("Failed to load config: {}", err);
        Config::default()
    })
});

#[derive(Serialize, Deserialize, Debug)]
pub struct Connection {
    pub port: i32,                       // The port the local client is running on
    pub address: String, // The address the local client is on, should always be localhost
    pub disable_auto_connect: bool,   // Do not attempt to connect to local client
    pub reconnect_interval_seconds: i32, // How many seconds between each reconnection attempt to the local client
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mods {
    pub disable_ddmk: bool,          // Stop DDMK from loading
    pub disable_crimson: bool,       // Stop Crimson from loading
    pub disable_ddmk_hooks: bool, // Stop DDMK hooks from being loaded, this does not stop hash verification though
    pub disable_crimson_hooks: bool, // Stop Crimson hooks from being loaded
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub connections: Connection,
    pub mods: Mods,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            connections: Connection {
                port: 21705,
                address: "localhost".to_string(),
                disable_auto_connect: false,
                reconnect_interval_seconds: 10,
            },
            mods: Mods {
                disable_ddmk: false,
                disable_crimson: false,
                disable_ddmk_hooks: false,
                disable_crimson_hooks: false,
            },
        }
    }
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    if !fs::exists("archipelago")? {
        fs::create_dir("archipelago/")?;
    }
    let config_path = "archipelago/randomizer.toml";
    if !Path::new(config_path).exists() {
        log::debug!("Config file not found. Creating a default one.");
        let toml_string =
            toml::to_string(&Config::default()).expect("Could not serialize default config");
        fs::write(config_path, toml_string).expect("Could not write default config to file");
    }
    match Figment::new()
        .merge(Toml::file(config_path))
        .extract::<Config>()
    {
        Ok(config) => Ok(config),
        Err(err) => {
            log::warn!("Failed to parse config: {err}. Backing up and regenerating.");

            let backup_path = "archipelago/randomizer.old.toml";
            fs::rename(config_path, backup_path)?;
            log::info!("Old config backed up to {:?}", backup_path);
               
            let toml_string =
                toml::to_string(&Config::default()).expect("Could not serialize default config");
            fs::write(config_path, &toml_string)?;

            Ok(Config::default())
        }
    }
}
