use crate::archipelago::get_connected;
use crate::hook::modify_item_table;
use crate::{archipelago, constants, generated_locations, hook};
use anyhow::anyhow;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, from_value};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const MAPPINGS_FILENAME: &str = "mappings.json";

static MAPPING: OnceLock<Mutex<Option<Mapping>>> = OnceLock::new();

fn default_gun() -> String {
    "Ebony & Ivory".to_string()
}

fn default_melee() -> String {
    "Rebellion".to_string()
}

/// Converts the option number from the slot data into a more usable gun name
fn parse_gun_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Value::deserialize(deserializer)?;
    match val {
        Value::Number(n) => match n.as_i64().unwrap_or_default() {
            0 => Ok("Ebony & Ivory".to_string()),
            1 => Ok("Shotgun".to_string()),
            2 => Ok("Artemis".to_string()),
            3 => Ok("Spiral".to_string()),
            4 => Ok("Kalina Ann".to_string()),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid gun number: {}",
                n
            ))),
        },
        Value::String(s) => Ok(s),
        other => Err(serde::de::Error::custom(format!(
            "Unexpected type: {:?}",
            other
        ))),
    }
}

/// Converts the option number from the slot data into a more usable melee name
fn parse_melee_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Value::deserialize(deserializer)?;
    match val {
        Value::Number(n) => match n.as_i64().unwrap_or_default() {
            0 => Ok("Rebellion (Normal)".to_string()),
            1 => Ok("Cerberus".to_string()),
            2 => Ok("Agni and Rudra".to_string()),
            3 => Ok("Nevan".to_string()),
            4 => Ok("Beowulf".to_string()),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid melee number: {}",
                n
            ))),
        },
        Value::String(s) => Ok(s),
        other => Err(serde::de::Error::custom(format!(
            "Unexpected type: {:?}",
            other
        ))),
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Mapping {
    // For mapping JSON
    pub seed: String,
    pub slot: String,
    pub items: HashMap<String, LocationData>,
    pub starter_items: Vec<String>,
    pub players: Vec<String>,
    pub adjudicators: HashMap<String, AdjudicatorData>,
    #[serde(default = "default_gun")]
    #[serde(deserialize_with = "parse_gun_number")]
    pub starter_gun: String,
    #[serde(default = "default_melee")]
    #[serde(deserialize_with = "parse_melee_number")]
    pub starter_melee: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AdjudicatorData {
    pub weapon: String,
    pub ranking: u8,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LocationData {
    pub item_name: String,
    pub description: String,
}

pub fn get_mappings() -> &'static Mutex<Option<Mapping>> {
    MAPPING.get_or_init(|| Mutex::new(None))
}

pub fn use_mappings() {
    match get_mappings().lock() {
        Ok(mapping_opt) => match mapping_opt.as_ref() {
            None => log::error!("No mapping found"),
            Some(mapping_data) => {
                // Run through each mapping entry
                for (location_name, location_data) in mapping_data.items.iter() {
                    // Acquire the default location data for a specific location
                    match generated_locations::ITEM_MISSION_MAP.get(location_name as &str) {
                        Some(entry) => match constants::get_item_id(&*location_data.item_name) {
                            // With the offset acquired, before the necessary replacement
                            Some(id) => unsafe {
                                if archipelago::location_is_checked_and_end(location_name) {
                                    // If the item procs an end mission event, replace with a dummy ID in order to not immediately trigger a mission end
                                    modify_item_table(entry.offset, hook::DUMMY_ID)
                                } else {
                                    // Replace the item ID with the new one
                                    modify_item_table(entry.offset, id)
                                }
                            },
                            None => {
                                log::warn!("Item not found: {}", location_data.item_name);
                            }
                        },
                        None => {
                            log::warn!("Location not found: {}", location_name);
                        }
                    }
                }
            }
        },
        Err(e) => {
            log::error!("Mapping error: {}", e);
        }
    }
}

/// Load mappings from a mappings file in the game's main directory
pub fn load_mappings_file() -> Result<Mapping, Box<dyn std::error::Error>> {
    if Path::new(MAPPINGS_FILENAME).try_exists()? {
        log::info!("Mapping file exists, loading");
        let mut json_reader =
            serde_json::Deserializer::from_reader(BufReader::new(File::open(MAPPINGS_FILENAME)?));
        Ok(Mapping::deserialize(&mut json_reader)?)
    } else {
        Err(Box::from(anyhow!("Mapping file doesn't exist")))
    }
}

pub(crate) fn parse_slot_data() -> Result<(), Box<dyn std::error::Error>> {
    let connected = get_connected().try_lock()?;
    let mappings: Mapping = from_value(connected.slot_data.clone())?;
    *get_mappings().lock().unwrap() = Some(mappings);
    Ok(())
}
