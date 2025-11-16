use crate::check_handler::Location;
use crate::constants::{EventCode, ItemCategory, DUMMY_ID, EVENT_TABLES, ITEM_ID_MAP, REMOTE_ID};
use crate::data::generated_locations;
use crate::game_manager::get_mission;
use crate::{constants, game_manager, mapping, utilities};
use anyhow::anyhow;
use std::error::Error;
use crate::archipelago::CHECKED_LOCATIONS;

/// If we are in a room with a key item+appropriate mission, return Ok(location_key)
pub fn in_key_item_room() -> Result<&'static str, Box<dyn Error>> {
    game_manager::with_session_read(|s| {
        for (location_key, item_entry) in generated_locations::ITEM_MISSION_MAP.iter() {
            if constants::get_items_by_category(ItemCategory::Key)
                .contains(&constants::get_item_name(item_entry.item_id))
            {
                if s.room == item_entry.room_number && s.mission == item_entry.mission {
                    return Ok(*location_key);
                }
            }
        }
        Err(Box::from(anyhow!("Not a key item room")))
    })
    .unwrap()
}

pub fn get_location_name_by_data(location_data: &Location) -> Result<&'static str, Box<dyn Error>> {
    if location_data.room == -1 {
        let mission_loc: Vec<_> = generated_locations::ITEM_MISSION_MAP.iter().filter(|(key, _item_entry)| {
            *(*key) == format!("Mission #{} Complete", location_data.mission).as_str()
        }).collect();
        return Ok(mission_loc[0].0)
    }
    let filtered_locs =
        generated_locations::ITEM_MISSION_MAP
            .iter()
            .filter(|(_key, item_entry)| {
                (item_entry.room_number == location_data.room)
                    && ((!item_entry.coordinates.has_coords())
                        || item_entry.coordinates == location_data.coordinates)
            });
    for (key, entry) in filtered_locs {
        if entry.item_id == location_data.item_id
            || location_data.item_id == *REMOTE_ID
            || location_data.item_id == *DUMMY_ID
        {
            return Ok(key);
        }
    }
    Err(Box::from("No location found"))
}

pub fn get_mapped_item_id(location_name: &str) -> Result<u32, Box<dyn Error>> {
    let mapping_data = mapping::MAPPING.read()?;
    let Some(mapping_data) = mapping_data.as_ref() else {
        return Err(Box::from("No mapping data"));
    };
    let id = mapping_data.items.get(location_name).unwrap().get_in_game_id::<constants::DMC3Config>();
    // To set the displayed graphic to the corresponding weapon
    if id > 0x39 {
        return Ok(match id {
            (0x40..0x44) => *ITEM_ID_MAP.get("Rebellion").unwrap(),
            0x44 => *ITEM_ID_MAP.get("Cerberus").unwrap(),
            0x45 => *ITEM_ID_MAP.get("Cerberus").unwrap(),
            (0x46..0x4A) => *ITEM_ID_MAP.get("Agni and Rudra").unwrap(),
            (0x4A..0x4F) => *ITEM_ID_MAP.get("Nevan").unwrap(),
            (0x4F..0x53) => *ITEM_ID_MAP.get("Beowulf").unwrap(),
            0x53 => *ITEM_ID_MAP.get("Ebony & Ivory").unwrap(),
            0x54 => *ITEM_ID_MAP.get("Shotgun").unwrap(),
            0x55 => *ITEM_ID_MAP.get("Artemis").unwrap(),
            0x56 => *ITEM_ID_MAP.get("Spiral").unwrap(),
            0x57 => *ITEM_ID_MAP.get("Kalina Ann").unwrap(),
            _ => {log::error!("Unrecognized id {}, default to Remote", id); *ITEM_ID_MAP.get("Remote").unwrap()},
        });
    }
    Ok(id)
}

pub fn edit_end_event(location_key: &str) {
    match EVENT_TABLES.get(&get_mission()) {
        None => {}
        Some(event_tables) => {
            for event_table in event_tables {
                if event_table.location == location_key {
                    for event in event_table.events.iter() {
                        if event.event_type == EventCode::End {
                            unsafe {
                                log::debug!(
                                    "Replaced END event at {:#X} with red orb",
                                    event.offset
                                );
                                if let Some(event_table_addr) = utilities::get_event_address() {
                                    utilities::replace_single_byte(
                                        event_table_addr + event.offset,
                                        0x00, // NOTE: This will fail if something like DDMK's arcade mode is used, due to the player having no officially picked up red orbs. But this shouldn't occur in normal gameplay.
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// If the location key corresponds to an END event and is checked off, return true, otherwise false
/// Used for dummy related item
pub(crate) fn location_is_checked_and_end(location_key: &str) -> bool {
    match EVENT_TABLES.get(&get_mission()) {
        None => false,
        Some(event_tables) => {
            for event_table in event_tables {
                if event_table.location == location_key {
                    for event in event_table.events.iter() {
                        if event.event_type == EventCode::End {
                            match CHECKED_LOCATIONS.read() {
                                Ok(checked_locations) => {
                                    if checked_locations.contains(&location_key) {
                                        return true;
                                    }
                                }
                                Err(err) => {
                                    log::error!("Failed to get checked locations: {}", err);
                                    return false;
                                }
                            }
                        }
                    }
                }
            }
            false
        }
    }
}
