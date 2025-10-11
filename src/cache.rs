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


pub struct DataPackageWrapper {
    pub dp: DataPackageObject,
    pub item_id_to_name: HashMap<String, HashMap<i64, String>>,
    pub location_id_to_name: HashMap<String, HashMap<i64, String>>,
}

impl DataPackageWrapper {
    fn new(dp: DataPackageObject) -> Self {
        let local_dp = dp.clone();
        // TODO Maybe I should try to only add entries when they are requested
        let item_id_to_name = {
            let mut game_map = HashMap::<String, HashMap<i64, String>>::new();
            for (game_name, data) in &local_dp.games {
                game_map.insert(game_name.clone(),
                                data
                                    .item_name_to_id
                                    .clone()
                                    .into_iter()
                                    .map(|(k, v)| (v, k))
                                    .collect()
                );
            }
            game_map
        };
        let location_id_to_name = {
            let mut game_map = HashMap::<String, HashMap<i64, String>>::new();
            for (game_name, data) in &local_dp.games {
                game_map.insert(game_name.clone(),
                                data
                                    .location_name_to_id
                                    .clone()
                                    .into_iter()
                                    .map(|(k, v)| (v, k))
                                    .collect()
                );
            }
            game_map
        };
        Self {
            dp,
            item_id_to_name,
            location_id_to_name,
        }
    }

}

pub(crate) static DATA_PACKAGE: LazyLock<RwLock<Option<DataPackageWrapper>>> =
    LazyLock::new(|| RwLock::new(None));

pub fn set_data_package(value: DataPackageObject) -> Result<(), Box<dyn Error>> {
    *DATA_PACKAGE.write()? = Some(DataPackageWrapper::new(value));
    Ok(())
}
