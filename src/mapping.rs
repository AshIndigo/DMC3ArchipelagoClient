use crate::data::generated_locations;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::constants::{Difficulty, Rank};
use archipelago_rs::{Client, CreateAsHint, Error, LocatedItem, Location};
use oneshot::Receiver;
use randomizer_utilities::APVersion;
use std::sync::{LazyLock, RwLock};
use std::thread;

pub static MAPPING: LazyLock<RwLock<Option<Mapping>>> = LazyLock::new(|| RwLock::new(None));

fn default_goal() -> Goal {
    Goal::Standard
}

fn default_difficulty_list() -> Vec<Difficulty> {
    vec![Difficulty::Easy, Difficulty::Normal]
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

/// Parse rank value
fn parse_rank<'de, D>(deserializer: D) -> Result<Rank, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Value::deserialize(deserializer)?;
    match val {
        Value::Number(n) => match Rank::from_repr(n.as_i64().unwrap_or_default() as usize) {
            None => Err(serde::de::Error::custom(format!(
                "Invalid rank option: {}",
                n
            ))),
            Some(n) => Ok(n),
        },
        other => Err(serde::de::Error::custom(format!(
            "Unexpected type: {:?}",
            other
        ))),
    }
}

/// Parse difficulty value
fn parse_difficulty<'de, D>(deserializer: D) -> Result<Difficulty, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Value::deserialize(deserializer)?;
    match val {
        Value::Number(n) => match Difficulty::from_repr(n.as_i64().unwrap_or_default() as usize) {
            None => Err(serde::de::Error::custom(format!(
                "Invalid difficulty option: {}",
                n
            ))),
            Some(n) => Ok(n),
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
    pub starter_items: Vec<String>,
    pub adjudicators: Option<HashMap<String, AdjudicatorData>>,
    pub start_melee: u8,
    pub start_second_melee: u8,
    pub start_gun: u8,
    pub start_second_gun: u8,
    pub randomize_skills: bool,
    pub randomize_gun_levels: bool,
    pub randomize_styles: bool,
    pub purple_orb_mode: bool,
    pub devil_trigger_mode: bool,
    pub check_ss_difficulty: bool,
    pub shop_checks: bool,
    #[serde(deserialize_with = "parse_death_link")]
    pub death_link: DeathlinkSetting,
    #[serde(default = "default_goal")]
    #[serde(deserialize_with = "parse_goal")]
    pub goal: Goal,
    pub mission_order: Option<Vec<u8>>,
    pub generated_version: Option<APVersion>,
    pub client_version: Option<APVersion>,
    #[serde(default)]
    #[serde(deserialize_with = "parse_rank")]
    pub mission_clear_rank: Rank,
    #[serde(default)]
    #[serde(deserialize_with = "parse_difficulty")]
    pub mission_clear_difficulty: Difficulty,
    #[serde(default = "default_difficulty_list")]
    pub initially_unlocked_difficulties: Vec<Difficulty>,
}

impl Mapping {
    /// Takes a mission and returns its index in mission_order
    pub(crate) fn get_index_for_mission(&self, mission: u32) -> usize {
        if let Some(order) = &self.mission_order {
            for (i, mis_id) in order.iter().enumerate() {
                if *mis_id as u32 == mission {
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

pub(crate) fn parse_slot_data(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    let mapping: Mapping = serde_path_to_error::deserialize(client.slot_data().clone())?;
    log::debug!("Mod version: {}", env!("CARGO_PKG_VERSION"));
    log::debug!(
        "Client version: {}",
        if let Some(cv) = mapping.client_version {
            cv.to_string()
        } else {
            "Unknown".to_string()
        }
    );
    log::debug!(
        "Generated version: {}",
        if let Some(gv) = mapping.generated_version {
            gv.to_string()
        } else {
            "Unknown".to_string()
        }
    );
    MAPPING.write()?.replace(mapping);
    Ok(())
}

pub static CACHED_LOCATIONS: LazyLock<RwLock<HashMap<String, LocatedItem>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn run_scouts_for_mission(client: &mut Client, mission: u32, hint: CreateAsHint) {
    run_scouts(client.scout_locations(get_locations_by_mission(client, mission), hint));
}
pub fn run_scouts_for_secret_mission(client: &mut Client) {
    run_scouts(client.scout_locations(get_secret_missions(client), CreateAsHint::No));
}

fn run_scouts(future: Receiver<Result<Vec<LocatedItem>, Error>>) {
    thread::spawn(|| match future.recv() {
        Ok(scouted) => {
            parse_scouts(scouted);
        }
        Err(err) => log::error!("Failed to run Scouts for: {}", err),
    });
}

pub fn parse_scouts(res: Result<Vec<LocatedItem>, Error>) {
    match res {
        Ok(items) => match CACHED_LOCATIONS.write() {
            Ok(mut cached_locations) => {
                for item in items {
                    cached_locations.insert(item.location().name().to_string(), item);
                }
            }
            Err(err) => {
                log::error!("Unable to write to location cache: {}", err)
            }
        },
        Err(err) => {
            log::error!("Failed to scout: {}", err);
        }
    }
}

pub fn get_locations_by_mission(client: &Client, mission: u32) -> Vec<Location> {
    let current_game = client.this_game();
    generated_locations::ITEM_MISSION_MAP
        .iter()
        .filter(|(_k, v)| v.mission == mission)
        .filter_map(|(k, _v)| current_game.location_by_name(*k))
        .collect()
}

pub fn get_secret_missions(client: &Client) -> Vec<Location> {
    let current_game = client.this_game();
    generated_locations::ITEM_MISSION_MAP
        .iter()
        .filter(|(k, _v)| k.contains("Secret Mission"))
        .filter_map(|(k, _v)| current_game.location_by_name(*k))
        .collect()
}
