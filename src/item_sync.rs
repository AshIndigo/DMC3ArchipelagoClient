use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;
use std::sync::atomic::AtomicI64;

const SYNC_FILE: &str = "archipelago.json";
//pub static SYNC_DATA: OnceLock<Mutex<SyncData>> = OnceLock::new();
pub static CURRENT_INDEX: AtomicI64 = AtomicI64::new(0);

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct SyncData {
    // Each save slot has its own sync index and offline checks
    pub room_sync_info: HashMap<String, [SlotSyncInfo; 10]>, // String is "seed_slot-name"
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct SlotSyncInfo {
    pub sync_index: i64,
    pub offline_checks: Vec<i64>,
}

pub fn write_sync_data_file(data: SyncData) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(SYNC_FILE)?;
    log::debug!("Writing sync file");
    file.write_all(
        serde_json::to_string_pretty(&data)?
            .as_bytes(),
    )?;
    file.flush()?;
    Ok(())
}

pub fn check_for_sync_file() -> bool {
    Path::new(SYNC_FILE).try_exists().unwrap_or_else(|err| {
        log::info!("Failed to check for sync file: {}", err);
        false
    })
}

/// Reads the received items indices from the save file
pub fn read_save_data() -> Result<SyncData, Box<dyn Error>> {
    if !check_for_sync_file() {
        Ok(SyncData::default())
    } else {
        let save_data = SyncData::deserialize(&mut serde_json::Deserializer::from_reader(
            BufReader::new(File::open(SYNC_FILE)?),
        ))?;
        Ok(save_data)
    }
}

/// Get the key for [SYNC_DATA]
pub fn get_sync_file_key(seed_name: &str, slot_name: String) -> String {
    format!("{}_{}", seed_name, slot_name)
}

/*/// Adds an offline location to be sent when room connection is restored
pub fn add_offline_check(location: i64, index: String, current_save_index: usize) -> Result<(), Box<dyn Error>> {
    let mut sync_data = get_sync_data().lock()?;
    if sync_data.room_sync_info.contains_key(&index) {
        sync_data
            .room_sync_info
            .get_mut(&index)
            .unwrap()
            .offline_checks
            .push(location);
    } else {
        sync_data
            .room_sync_info
            .insert(index, SlotSyncInfo::default());
    }
    write_sync_data_file()?;
    Ok(())
}*/

/*pub fn send_offline_checks(client: &mut Client, index: String) -> Result<(), Box<dyn Error>> {
    log::debug!("Attempting to send offline checks");
    let mut sync_data = get_sync_data().lock()?;
    if sync_data.room_sync_info.contains_key(&index) {
        match client.mark_checked(
            sync_data
                .room_sync_info
                .get(&index)
                .unwrap()
                .offline_checks
                .clone(),
        ) {
            Ok(_) => {
                log::info!("Successfully sent offline checks");
                sync_data
                    .room_sync_info
                    .get_mut(&index)
                    .unwrap()
                    .offline_checks
                    .clear();
                write_sync_data_file()?;
            }
            Err(err) => {
                log::error!(
                    "Failed to send offline checks, will attempt next reconnection: {}",
                    err
                );
            }
        }
    }
    Ok(())
}*/