use anyhow::Error;
use archipelago_rs::protocol::{DataPackageObject, RoomInfo};
use serde::{Deserialize};
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;

pub const CACHE_FILENAME: &str = "cache.json";

/// Checks for the Archipelago RoomInfo cache file
/// If file exists then check the checksums in it
/// Returns false if file doesn't exist (or if it cant be checked for)
pub fn check_for_cache_file() -> bool {
    Path::new(CACHE_FILENAME).try_exists().unwrap_or_else(|err| {
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
                    for name in &room_info.games { // For all games in room
                        if data_package_object.games.contains_key(name) { // See if cache file has game
                            if *room_info.datapackage_checksums.get(name)? != data_package_object.games.get(name)?.checksum {
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
    let mut file = File::create(CACHE_FILENAME)?;
    file.write_all(serde_json::to_string_pretty(&data)?.as_bytes())?;
    file.flush()?;
    Ok(())
}

pub(crate) fn read_cache() -> Result<DataPackageObject, Error> {
    let cache = DataPackageObject::deserialize(&mut serde_json::Deserializer::from_reader(
        BufReader::new(File::open(CACHE_FILENAME)?),
    ))?;
    Ok(cache)
}
