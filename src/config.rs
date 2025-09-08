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
    pub offline: bool,   // Do not attempt to connect to local client
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
    pub force_enable_egui: bool, // To forcibly re-enable the EGUI window
}

impl Default for Config {
    fn default() -> Config {
        Config {
            connections: Connection {
                port: 21705,
                address: "localhost".to_string(),
                offline: false,
                reconnect_interval_seconds: 10,
            },
            mods: Mods {
                disable_ddmk: false,
                disable_crimson: false,
                disable_ddmk_hooks: false,
                disable_crimson_hooks: false,
            },
            force_enable_egui: false,
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
    // TODO If the config is missing a key (i.e I updated it, I probably should go ahead and rename the old one and regenerate a new one)
    match Figment::new()
        .merge(Toml::file(config_path))
        .extract::<Config>()
    {
        Ok(config) => Ok(config),
        Err(err) => Err(Box::new(err)),
    }
}
