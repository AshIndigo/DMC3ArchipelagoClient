use std::sync::OnceLock;
use std::path::Path;
use std::io::BufReader;
use std::fs::File;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::{archipelago, constants, generated_locations, hook};
use crate::hook::modify_itm_table;

const MAPPINGS_FILENAME: &str = "mappings.json";

pub static MAPPING: OnceLock<Mapping> = OnceLock::new();

#[derive(Deserialize, Serialize, Debug)]
pub struct Mapping {
    // For mapping JSON
    pub seed: String,
    pub slot: String,
    pub items: HashMap<String, LocationData>,
    pub starter_items: Vec<String>,
    pub players: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LocationData {
    pub name: String,
    pub description: String,
}

pub fn use_mappings() {
    // TODO Need to see if the provided seed matches up with the world seed or something to ensure mappings are correct
    match MAPPING.get() {
        Some(data) => {
            for (location_name, location_data) in data.items.iter() {
                match generated_locations::ITEM_MISSION_MAP.get(location_name as &str) {
                    Some(entry) => match constants::get_item_id(&*location_data.name) {
                        Some(id) => unsafe {
                            if archipelago::location_is_checked_and_end(location_name) {
                                modify_itm_table(entry.offset, hook::DUMMY_ID)
                            } else {
                                modify_itm_table(entry.offset, id)
                            }
                        },
                        None => {
                            log::warn!("Item not found: {}", location_data.name);
                        }
                    },
                    None => {
                        log::warn!("Location not found: {}", location_name);
                    }
                }
            }
        }
        None => {
            log::error!("No mapping found");
        }
    }
}

pub fn load_mappings_file() -> Result<Mapping, Box<dyn std::error::Error>> {
    if Path::new(MAPPINGS_FILENAME).try_exists()? {
        log::info!("Mapping file Exists!");
        let mut json_reader =
            serde_json::Deserializer::from_reader(BufReader::new(File::open(MAPPINGS_FILENAME)?));
        Ok(Mapping::deserialize(&mut json_reader)?)
    } else {
        Err(Box::from(anyhow!("Mapping file doesn't exist")))
    }
}