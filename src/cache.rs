use anyhow::Error;
use archipelago_rs::protocol::RoomInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;

/// Checks for the Archipelago RoomInfo cache file
/// If file exists then check the checksums in it
/// Returns false if file doesn't exist (or if it cant be checked for)
pub fn check_for_cache_file() -> bool {
    Path::new("cache.json").try_exists().unwrap_or_else(|err| {
        log::info!("Failed to check for cache file: {}", err);
        false
    })
}

#[derive(Deserialize, Serialize)]
struct Cache {
    checksums: HashMap<String, String>,
    data_package: HashMap<String, CustomGameData>,
}

/// Check the cached checksums with the stored file. Return any that do not match
pub async fn find_checksum_errors(room_info: &RoomInfo) -> Option<Vec<String>> {
    let file = File::open("cache.json");
    match file {
        Ok(cache) => {
            let json = Cache::deserialize(&mut serde_json::Deserializer::from_reader(BufReader::new(cache)));
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

#[derive(Deserialize, Serialize, Debug)]
pub struct CustomGameData {
    pub item_name_to_id: HashMap<String, i32>,
    pub location_name_to_id: HashMap<String, i32>,
}

/// Write the DataPackage to a JSON file
pub async fn write_cache(
    data: HashMap<String, CustomGameData>,
    room_info: &RoomInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create("cache.json")?;
    let cache: Cache = Cache {
        checksums: room_info.datapackage_checksums.clone(),
        data_package: data,
    };
    file.write_all(serde_json::to_string_pretty(&cache)?.as_bytes())?;
    file.flush()?;
    Ok(())
}

pub(crate) fn read_cache() -> Result<CustomGameData, Error> {
    let cache = Cache::deserialize(&mut serde_json::Deserializer::from_reader(BufReader::new(
        File::open("cache.json")?,
    )))?;
    Ok(CustomGameData {
        item_name_to_id: cache
            .data_package
            .get("Devil May Cry 3")
            .unwrap()
            .item_name_to_id
            .clone(),
        location_name_to_id: cache
            .data_package
            .get("Devil May Cry 3")
            .unwrap()
            .location_name_to_id
            .clone(),
    })
}
