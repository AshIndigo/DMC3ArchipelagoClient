//! PopTracker related stuff

use crate::constants::{Difficulty, GUN_NAMES, Style};
use crate::data::game_structs::{ActiveMissionActorData, GameData, SessionData, TotalRankings};
use crate::game_manager::{get_mission, get_room};
use crate::hooks::hook::calculate_max_mission;
use crate::mapping::{MAPPING, Mapping, ModModeData};
use archipelago_rs::{BounceOptions, Client, DataStorageOperation};
use serde::Serialize;
use serde_json::{json, to_value};
use std::collections::HashMap;
use strum::IntoEnumIterator;

trait HasKey {
    const KEY: &'static str;
}

#[derive(Serialize)]
/// Sent when the player changes room
// Use for Bounce messages
pub struct RoomUpdate {
    pub mission: u32,
    pub room: i32,
}

impl HasKey for RoomUpdate {
    const KEY: &'static str = "RoomUpdate";
}

impl RoomUpdate {
    fn new(select_mission_screen: bool) -> Self {
        RoomUpdate {
            mission: if !select_mission_screen {
                get_mission()
            } else {
                0
            },
            room: if !select_mission_screen {
                get_room()
            } else {
                0
            },
        }
    }
}

#[derive(Serialize)]
#[serde(transparent)]
/// Updates which missions are unlocked?
pub struct AvailableMissions {
    /// See [Difficulty] for order
    pub available_missions: HashMap<&'static str, Vec<u8>>,
}

impl HasKey for AvailableMissions {
    const KEY: &'static str = "MaxMissions";
}

impl AvailableMissions {
    fn new(mapping: &Mapping) -> Self {
        let mut map = HashMap::new();
        for diff in Difficulty::iter() {
            map.insert(diff.into(), get_unlocked_missions(diff, mapping));
        }
        AvailableMissions {
            available_missions: map,
        }
    }
}

// This includes the newest non-completed mission
fn get_unlocked_missions(difficulty: Difficulty, mapping: &Mapping) -> Vec<u8> {
    let mut res = vec![];
    const NOT_COMPLETED: u8 = 0xFF;
    // Check the rankings, this is how we know what missions are available
    TotalRankings::with_read(|r| {
        let rankings = r.get_ranking_for_difficulty(difficulty);
        for (ind, ranking) in rankings.iter().enumerate() {
            if *ranking != NOT_COMPLETED {
                res.push(1 + (ind as u8));
            }
        }
    })
    .unwrap();
    res.push(calculate_max_mission(mapping, difficulty));
    res
}

#[derive(Serialize)]
#[serde(transparent)]
/// Sent when a gun is updated
pub struct GunLevels {
    pub gun_levels: HashMap<&'static str, u32>,
}

impl HasKey for GunLevels {
    const KEY: &'static str = "GunLevels";
}

impl GunLevels {
    fn new(current_levels: [u32; 5]) -> Self {
        GunLevels {
            gun_levels: GUN_NAMES.iter().copied().zip(current_levels).collect(),
        }
    }

    pub(crate) fn update(
        gun_idx: usize,
        level: u8,
        client: &mut Client<ModModeData>,
    ) -> Result<(), archipelago_rs::Error> {
        match to_value(Self::new(Default::default())) {
            Ok(ser) => {
                let mut map = HashMap::new();
                map.insert(GUN_NAMES[gun_idx].to_string(), json!(level));
                client.change(
                    format!("{}_{}", Self::KEY, client.this_player().name()),
                    ser,
                    [DataStorageOperation::Update(map)],
                    false,
                )?;
                Ok(())
            }
            Err(err) => Err(archipelago_rs::Error::Serialize(err)),
        }
    }
}

#[derive(Serialize)]
#[serde(transparent)]
/// Sent when a Style levels up
pub struct StyleLevels {
    pub style_levels: HashMap<&'static str, u32>,
}

impl HasKey for StyleLevels {
    const KEY: &'static str = "StyleLevels";
}

impl StyleLevels {
    // Ignores Quicksilver and Doppelgänger
    pub(crate) fn new(style_levels: [u32; 6]) -> Self {
        StyleLevels {
            style_levels: Style::INTERNAL_ORDER
                .iter()
                .map(|g| g.into())
                .zip(style_levels)
                .collect(),
        }
    }

    /// Updates the style level in storage
    pub(crate) fn update(
        style: Style,
        level: u32,
        client: &mut Client<ModModeData>,
    ) -> Result<(), archipelago_rs::Error> {
        match to_value(Self::new([0; 6])) {
            Ok(ser) => {
                let mut map = HashMap::new();
                map.insert(style.to_string(), json!(level));
                client.change(
                    format!("{}_{}", Self::KEY, client.this_player().name()),
                    ser,
                    [DataStorageOperation::Update(map)],
                    false,
                )?;
                Ok(())
            }
            Err(err) => Err(archipelago_rs::Error::Serialize(err)),
        }
    }
}

#[derive(Serialize)]
#[serde(transparent)]
/// Sent when a skill is bought
pub struct SkillUpdate {
    // This one is going to be interesting
    pub skills: [u32; 8],
}

impl HasKey for SkillUpdate {
    const KEY: &'static str = "SkillUpdate";
}

impl SkillUpdate {
    fn new() -> Self {
        // If MissionActorData is available use that, otherwise use SessionData
        if let Ok(sk) = ActiveMissionActorData::with_read(|a| SkillUpdate {
            skills: a.expertise,
        }) {
            return sk;
        }
        if let Ok(sk) = SessionData::with_read(|a| SkillUpdate {
            skills: a.expertise,
        }) {
            return sk;
        }
        SkillUpdate { skills: [0; 8] }
    }
    // TODO Need to implement an update() for this
}

fn update_data_storage<T>(
    value: T,
    client: &mut Client<ModModeData>,
) -> Result<(), archipelago_rs::Error>
where
    T: Serialize + HasKey,
{
    match to_value(value) {
        Ok(ser) => client.set(
            format!(
                "{}_{}", // DataType_SlotName
                T::KEY,
                client.this_player().name()
            ),
            ser,
            false,
        ),
        Err(err) => Err(archipelago_rs::Error::Serialize(err)),
    }
}

// Set all keys upon connection to match the currently loaded save. Needs SessionData available obviously
pub fn initial_connection_updates(
    client: &mut Client<ModModeData>,
) -> Result<(), archipelago_rs::Error> {
    if let Ok(map) = MAPPING.read()
        && let Some(mapping) = map.as_ref()
    {
        // Unlocked missions, always set
        update_data_storage(AvailableMissions::new(mapping), client)?;
        if SessionData::with_read(|s| -> Result<(), archipelago_rs::Error> {
            // Add extra information if the respective elements aren't randomized
            if !mapping.randomize_gun_levels {
                update_data_storage(GunLevels::new(s.ranged_weapon_levels), client)?;
            }
            if !mapping.randomize_styles {
                update_data_storage(StyleLevels::new(s.style_levels), client)?;
            }
            if !mapping.randomize_skills {
                update_data_storage(SkillUpdate::new(), client)?;
            }
            Ok(())
        })
        .is_err()
        {
            log::error!("Session Data not available, can't set data storage state");
        }
    }

    Ok(())
}

/// Send out a bounce whenever we change room
pub fn send_room_transition(
    client: &mut Client<ModModeData>,
    select_screen: bool,
) -> Result<(), archipelago_rs::Error> {
    match to_value(RoomUpdate::new(select_screen)) {
        Ok(ser) => client.bounce(
            ser,
            BounceOptions::new().slots([client.this_player().slot()]),
        ),
        Err(err) => Err(archipelago_rs::Error::Serialize(err)),
    }
}
