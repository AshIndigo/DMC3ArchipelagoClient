use crate::data::generated_locations;
use crate::hook::modify_item_table;
use crate::{constants, location_handler};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use std::sync::{LazyLock, RwLock};
use randomizer_utilities::APVersion;
use randomizer_utilities::archipelago_utilities::CONNECTED;
use randomizer_utilities::mapping_utilities::LocationData;

pub static MAPPING: LazyLock<RwLock<Option<Mapping>>> = LazyLock::new(|| RwLock::new(None));

fn default_gun() -> String {
    "Ebony & Ivory".to_string()
}

fn default_melee() -> String {
    "Rebellion".to_string()
}

fn default_goal() -> Goal {
    Goal::Standard
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
            0 => Ok("Rebellion".to_string()),
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

/// Figure out which DL setting were on
fn parse_death_link<'de, D>(deserializer: D) -> Result<DeathlinkSetting, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Value::deserialize(deserializer)?;
    match val {
        Value::Number(n) => match n.as_i64().unwrap_or_default() {
            0 => Ok(DeathlinkSetting::Off),
            1 => Ok(DeathlinkSetting::DeathLink),
            2 => Ok(DeathlinkSetting::HurtLink),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid DL option: {}",
                n
            ))),
        },
        other => Err(serde::de::Error::custom(format!(
            "Unexpected type: {:?}",
            other
        ))),
    }
}

/// Parse which goal we are on
fn parse_goal<'de, D>(deserializer: D) -> Result<Goal, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Value::deserialize(deserializer)?;
    match val {
        Value::Number(n) => match n.as_i64().unwrap_or_default() {
            0 => Ok(Goal::Standard),
            1 => Ok(Goal::All),
            2 => Ok(Goal::RandomOrder),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid goal option: {}",
                n
            ))),
        },
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
    pub items: HashMap<String, LocationData>,
    pub starter_items: Vec<String>,
    pub adjudicators: Option<HashMap<String, AdjudicatorData>>,
    #[serde(default = "default_melee")]
    #[serde(deserialize_with = "parse_melee_number")]
    pub start_melee: String,
    #[serde(default = "default_gun")]
    #[serde(deserialize_with = "parse_gun_number")]
    pub start_gun: String,
    pub randomize_skills: bool,
    pub randomize_styles: bool,
    pub purple_orb_mode: bool,
    pub devil_trigger_mode: bool,
    #[serde(deserialize_with = "parse_death_link")]
    pub death_link: DeathlinkSetting,
    #[serde(default = "default_goal")]
    #[serde(deserialize_with = "parse_goal")]
    pub goal: Goal,
    pub mission_order: Option<Vec<u8>>,
    pub generated_version: Option<APVersion>,
    pub client_version: Option<APVersion>
}

impl Mapping {
    /// Takes a mission and returns its index in mission_order
    pub(crate) fn get_index_for_mission(&self, mission: u32) -> usize {
        if let Some(order) = &self.mission_order {
            for i in 0..order.len() {
                if order[i] as u32 == mission {
                    return i;
                }
            }
            (mission - 1) as usize
        } else {
            (mission - 1) as usize
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, strum_macros::Display)]
pub enum Goal {
    /// Beat M20 in linear order M1-M20 (Default)
    Standard,
    /// Beat all missions, all are unlocked at start
    All,
    /// Beat all missions in a randomized linear order
    RandomOrder,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum DeathlinkSetting {
    DeathLink, // Normal DeathLink Behavior
    HurtLink,  // Sends out DeathLink messages when you die. But only hurts you if you receive one
    Off,       // Don't send/receive DL related messages
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AdjudicatorData {
    pub weapon: String,
    pub ranking: u8,
}

pub fn use_mappings() -> Result<(), Box<dyn std::error::Error>> {
    let guard = MAPPING.read()?; // Annoying
    let mapping = guard.as_ref().ok_or("No mappings found, cannot use")?;
    // Run through each mapping entry
    for (location_name, _location_data) in mapping.items.iter() {
        // Acquire the default location data for a specific location
        match generated_locations::ITEM_MISSION_MAP.get(location_name as &str) {
            Some(entry) => {
                // With the offset acquired, before the necessary replacement
                if location_handler::location_is_checked_and_end(location_name) {
                    // If the item procs an end mission event, replace with a dummy ID in order to not immediately trigger a mission end
                    modify_item_table(entry.offset, *constants::DUMMY_ID as u8)
                }
            }
            None => {
                log::warn!("Location not found: {}", location_name);
            }
        }
    }
    Ok(())
}

pub(crate) fn parse_slot_data() -> Result<(), Box<dyn std::error::Error>> {
    match CONNECTED.read() {
        Ok(conn_opt) => {
            if let Some(connected) = conn_opt.as_ref() {
                MAPPING.write()?.replace(serde_path_to_error::deserialize(
                    connected.slot_data.clone(),
                )?);
                Ok(())
            } else {
                Err("No mapping found, cannot parse".into())
            }
        }
        Err(err) => Err(err.into()),
    }
}

