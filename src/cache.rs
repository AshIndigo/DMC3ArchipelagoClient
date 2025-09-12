use crate::constants::GAME_NAME;
use archipelago_rs::protocol::{DataPackageObject, RoomInfo};
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{LazyLock, RwLock};

pub const CACHE_FILENAME: &str = "cache.json";

/// Checks for the Archipelago RoomInfo cache file
/// If file exists then check the checksums in it
/// Returns false if file doesn't exist (or if it cant be checked for)
pub fn check_for_cache_file() -> bool {
    Path::new(CACHE_FILENAME)
        .try_exists()
        .unwrap_or_else(|err| {
            log::info!("Failed to check for cache file: {}", err);
            false
        })
}

/// Check the cached checksums with the stored file. Return any that do not match
pub async fn find_checksum_errors(room_info: &RoomInfo) -> Option<Vec<String>> {
    match File::open(CACHE_FILENAME) {
        Ok(cache_file) => {
            match DataPackageObject::deserialize(&mut serde_json::Deserializer::from_reader(
                BufReader::new(cache_file),
            )) {
                Ok(data_package_object) => {
                    let mut failed_checks = vec![];
                    for name in &room_info.games {
                        // For all games in room
                        if data_package_object.games.contains_key(name) {
                            // See if cache file has game
                            if *room_info.datapackage_checksums.get(name)?
                                != data_package_object.games.get(name)?.checksum
                            {
                                failed_checks.push(name.clone()); // Checksums do not match
                            }
                        } else {
                            failed_checks.push(name.clone()); // Cache file is missing game
                        }
                    }
                    if failed_checks.is_empty() {
                        None
                    } else {
                        Some(failed_checks)
                    }
                }
                Err(err) => {
                    log::info!("Failed to deserialize JSON: {}", err);
                    None
                }
            }
        }
        Err(err) => {
            log::info!("Failed to open cache file: {}", err);
            None
        }
    }
}

/// Write the DataPackage to a JSON file
pub async fn write_cache(data: &&&DataPackageObject) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(
        CACHE_FILENAME,
        serde_json::to_string_pretty(&data)?.as_bytes(),
    )?;
    Ok(())
}

pub(crate) fn read_cache() -> Result<DataPackageObject, Box<dyn Error>> {
    let cache = DataPackageObject::deserialize(&mut serde_json::Deserializer::from_reader(
        BufReader::new(File::open(CACHE_FILENAME)?),
    ))?;
    Ok(cache)
}

pub(crate) static DATA_PACKAGE: LazyLock<RwLock<Option<DataPackageObject>>> =
    LazyLock::new(|| RwLock::new(None));

pub(crate) static ITEM_ID_TO_NAME: LazyLock<RwLock<Option<HashMap<i64, String>>>> =
    LazyLock::new(|| RwLock::new(get_item_id_to_name()));

fn get_item_id_to_name() -> Option<HashMap<i64, String>> {
    if let Some(data_package) = DATA_PACKAGE.read().unwrap().as_ref() {
        Some(
            data_package
                .games
                .get(GAME_NAME)
                .unwrap()
                .item_name_to_id
                .clone()
                .into_iter()
                .map(|(k, v)| (v, k))
                .collect(),
        )
    } else {
        None
    }
}

pub(crate) static LOCATION_ID_TO_NAME: LazyLock<RwLock<Option<HashMap<i64, String>>>> =
    LazyLock::new(|| RwLock::new(get_location_id_to_name()));

fn get_location_id_to_name() -> Option<HashMap<i64, String>> {
    if let Some(data_package) = DATA_PACKAGE.read().unwrap().as_ref() {
        Some(
            data_package
                .games
                .get(GAME_NAME)
                .unwrap()
                .location_name_to_id
                .clone()
                .into_iter()
                .map(|(k, v)| (v, k))
                .collect(),
        )
    } else {
        None
    }
}

pub fn set_data_package(value: DataPackageObject) -> Result<(), Box<dyn Error>> {
    *DATA_PACKAGE.write()? = Some(value);
    *ITEM_ID_TO_NAME.write()? = get_item_id_to_name();
    *LOCATION_ID_TO_NAME.write()? = get_location_id_to_name();
    Ok(())
}
