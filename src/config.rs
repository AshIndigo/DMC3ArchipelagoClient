use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    randomizer_utilities::load_config("dmc3_randomizer").unwrap_or_else(|err| {
        log::error!("Failed to load config: {}", err);
        Config::default()
    })
});

#[derive(Serialize, Deserialize, Debug)]
pub struct Connection {
    pub port: i32,                       // The port the local client is running on
    pub address: String, // The address the local client is on, should always be localhost
    pub disable_auto_connect: bool, // Do not attempt to connect to local client
    pub reconnect_interval_seconds: i32, // How many seconds between each reconnection attempt to the local client
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mods {
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
                disable_ddmk_hooks: false,
                disable_crimson_hooks: false,
            },
        }
    }
}
