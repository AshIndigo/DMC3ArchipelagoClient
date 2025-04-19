use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use archipelago_rs::protocol::RoomInfo;
use std::fs::File;
use std::io::{BufReader, Write};

/// Checks for the Archipelago RoomInfo cache file
/// If file exists then check the checksums in it
/// Returns false if file doesn't exist (or if it cant be checked for)
pub fn check_for_cache_file() -> bool {
    match Path::new("cache.json").try_exists() {
        Ok(res) => {
            if res == true {
                log::info!("Cache file Exists!");
                true
            } else {
                false
            }
        }
        Err(_) => {
            log::info!("Failed to check for cache file!");
            false
        }
    }
}

#[derive(Deserialize, Serialize)]
struct Cache {
    checksums: HashMap<String, String>,
    data_package: HashMap<String, CustomGameData>,
}

/// Check the cached checksums with the stored file. Return any that do not match
pub async fn check_checksums(room_info: &RoomInfo) -> Option<Vec<String>> {
    let file = File::open("cache.json");
    match file {
        Ok(cache) => {
            let reader = BufReader::new(cache);
            let mut json_reader = serde_json::Deserializer::from_reader(reader);
            let json = Cache::deserialize(&mut json_reader);
            match json {
                Ok(cac) => {
                    let mut failed_checks = vec![];
                    for key in cac.checksums.keys() {
                        if room_info.datapackage_checksums.get(key)
                            != cac.checksums.get(key.as_str())
                        {
                            failed_checks.push(key.clone());
                        }
                    }
                    if failed_checks.is_empty() {
                        None
                    } else {
                        Some(failed_checks)
                    }
                }
                Err(err) => { log::info!("Failed to deserialize JSON: {}", err); None },
            }
        }
        Err(err) => { log::info!("Failed to open cache file: {}", err); None },
    }
}

#[derive(Deserialize, Serialize)]
pub struct CustomGameData {
    pub item_name_to_id: HashMap<String, i32>,
    pub location_name_to_id: HashMap<String, i32>,
}

/// Write the DataPackage to a JSON file
// TODO Maybe don't let this panic if it fails to make the file?
pub async fn write_cache(
    data: HashMap<String, CustomGameData>,
    room_info: &RoomInfo,
) {
    let mut file = File::create("cache.json").expect("Failed to create cache file");
    let cache: Cache = Cache {
        checksums: room_info.datapackage_checksums.clone(),
        data_package: data,
    };
    file.write_all(serde_json::to_string_pretty(&cache).expect("Failed to convert cache struct to string").as_bytes()).expect("Failed to convert to bytes");
    file.flush().expect("Failed to flush cache file");
    log::info!("Writing cache");
}

pub(crate) fn read_cache() -> Option<CustomGameData> {
    let file = File::open("cache.json");
    match file {
        Ok(cache) => {
            let reader = BufReader::new(cache);
            let mut json_reader = serde_json::Deserializer::from_reader(reader);
            let json = Cache::deserialize(&mut json_reader);
            match json {
                Ok(cac) => {
                   Some(CustomGameData {
                       item_name_to_id: cac.data_package.get("Devil May Cry 3").unwrap().item_name_to_id.clone(),
                       location_name_to_id: cac.data_package.get("Devil May Cry 3").unwrap().location_name_to_id.clone(),
                   })
                }
                Err(err) => { log::info!("Failed to deserialize JSON: {}", err); None },
            }
        }
        Err(err) => { log::info!("Failed to open cache file: {}", err); None },
    }
}