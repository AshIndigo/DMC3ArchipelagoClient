use crate::archipelago::{CONNECTED, SLOT_NUMBER};
use crate::data::generated_locations;
use crate::hook::modify_item_table;
use crate::{cache, constants, location_handler};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::constants::REMOTE_ID;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, RwLock};

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
                "Invalid gun number: {}",
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
    pub adjudicators: HashMap<String, AdjudicatorData>,
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
    pub goal: Goal,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum Goal {
    Standard,    // Beat M20 in linear order M1-M20 (Default)
    All,         // Beat all missions, all are unlocked at start
    RandomOrder, // Beat all missions in a randomized linear order
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

#[derive(Deserialize, Serialize, Debug)]
pub struct LocationData {
    // Item name, used for descriptions
    #[serde(default)]
    item_id: Option<i32>,
    // Slot ID for recipient
    owner: i32,
}

impl LocationData {
    fn is_item_remote(&self) -> bool {
        self.owner != SLOT_NUMBER.load(Ordering::SeqCst)
    }

    pub fn get_in_game_id(&self) -> u32 {
        // Used for setting values in DMC3
        if self.is_item_remote() {
            *REMOTE_ID
        } else {
            match self.item_id {
                None => 0,
                Some(id) => id as u32,
            }
        }
    }

    pub(crate) fn get_item_name(&self) -> Result<String, Box<dyn std::error::Error>> {
        //let player_name = get_slot_name(self.owner)?;
        if let Some(cache) = (*cache::DATA_PACKAGE).read()?.as_ref() {
            let game_name = &{
                match CONNECTED.read().as_ref() {
                    Ok(con) => match &**con {
                        None => return Err("Connected is None".into()),
                        Some(connected) => match connected.slot_info.get(&self.owner) {
                            None => {
                                return Err(format!("Missing slot info for {}", self.owner).into());
                            }
                            Some(info) => info.game.clone(),
                        },
                    },
                    Err(_err) => {
                        return Err("PoisonError occurred when getting 'Connected'".into());
                    }
                }
            };
            match self.item_id {
                None => Err("Item ID is None, cannot get name".into()),
                Some(item_id) => match cache.item_id_to_name.get(game_name) {
                    None => Err(format!("{} does not exist in cache", game_name).into()),
                    Some(item_id_to_name) => match item_id_to_name.get(&(item_id as i64)) {
                        None => Err(format!(
                            "{:?} does not exist in {}'s item cache",
                            item_id, game_name
                        )
                        .into()),
                        Some(name) => Ok(name.clone()),
                    },
                },
            }
        } else {
            Err(Box::from("Data package is not here"))
        }
    }
    // Format a description
    pub fn get_description(&self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(format!(
            "{}'s {}",
            get_slot_name(self.owner)?,
            self.get_item_name()?
        ))
    }
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

pub fn get_own_slot_name() -> Result<String, Box<dyn std::error::Error>> {
    get_slot_name(SLOT_NUMBER.load(Ordering::SeqCst))
}

pub(crate) fn get_slot_name(slot: i32) -> Result<String, Box<dyn std::error::Error>> {
    let uslot = slot as usize;
    match CONNECTED.read() {
        Ok(conn_opt) => {
            if let Some(connected) = conn_opt.as_ref() {
                if slot == 0 {
                    return Ok("Server".to_string());
                }
                if (slot < 0) || (uslot - 1 >= connected.players.len()) {
                    return Err(format!("Slot index not valid: {}", slot).into());
                }
                Ok(connected.players[uslot - 1].name.clone())
            } else {
                Err("Not connected, cannot get name".into())
            }
        }
        Err(err) => Err(err.into()),
    }
}
