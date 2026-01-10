use crate::constants::{Coordinates, Difficulty, Rank, EMPTY_COORDINATES};
use crate::data::generated_locations;
use crate::game_manager::{get_mission, set_item, with_session_read};
use crate::mapping::MAPPING;
use crate::ui::text_handler;
use crate::utilities::{get_inv_address, DMC3_ADDRESS};
use crate::{constants, create_hook, game_manager, location_handler};
use minhook::{MinHook, MH_STATUS};
use std::cmp::PartialEq;
use std::fmt::{Display, Formatter};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use randomizer_utilities::read_data_from_address;

pub const ITEM_HANDLE_PICKUP_ADDR: usize = 0x1b45a0;
pub static ORIGINAL_HANDLE_PICKUP: OnceLock<unsafe extern "C" fn(item_struct: usize)> =
    OnceLock::new();

pub const ITEM_PICKED_UP_ADDR: usize = 0x1aa6e0;
pub static ORIGINAL_ITEM_PICKED_UP: OnceLock<
    unsafe extern "C" fn(loc_chk_id: usize, param_2: i16, item_id: i32),
> = OnceLock::new();

pub const RESULT_CALC_ADDR: usize = 0x2a0f10;
pub static ORIGINAL_RESULT_CALC: OnceLock<
    unsafe extern "C" fn(cuid_result: usize, ranking: i32) -> i32,
> = OnceLock::new();

pub fn setup_check_hooks() -> Result<(), MH_STATUS> {
    log::debug!("Setting up check related hooks");
    unsafe {
        create_hook!(
            ITEM_HANDLE_PICKUP_ADDR,
            item_non_event,
            ORIGINAL_HANDLE_PICKUP,
            "Non event item"
        );
        create_hook!(
            ITEM_PICKED_UP_ADDR,
            item_event,
            ORIGINAL_ITEM_PICKED_UP,
            "Event item"
        );
        create_hook!(
            RESULT_CALC_ADDR,
            mission_complete_check,
            ORIGINAL_RESULT_CALC,
            "Mission complete"
        );
        create_hook!(
            PURCHASE_ITEM_ADDR,
            purchase_item_check,
            ORIGINAL_PURCHASE_ITEM,
            "Purchase item from store"
        );
    }
    Ok(())
}

/// Hook into item handle method (1b45a0). Handles non-event item pick up locations
pub fn item_non_event(item_struct: usize) {
    unsafe {
        let base_ptr = item_struct as *const u8;
        let item_id_ptr = base_ptr.add(0x60) as *const i32; // Don't remove this
        let item_id = *(base_ptr.add(0x60));
        if is_valid_id(item_id as u32) {
            // Ignore red orbs
            if item_id < 0x3A {
                let x_coord_addr = item_id_ptr.offset(0x1);
                let y_coord_addr = item_id_ptr.offset(0x2);
                let z_coord_addr = item_id_ptr.offset(0x3);
                let x_coord_val = (*(x_coord_addr) as u32).to_be();
                let y_coord_val = (*(y_coord_addr) as u32).to_be();
                let z_coord_val = (*(z_coord_addr) as u32).to_be();
                let loc = Location {
                    location_type: LocationType::Standard,
                    item_id: item_id as u32,
                    room: game_manager::get_room(),
                    mission: get_mission(),
                    coordinates: Coordinates {
                        x: x_coord_val,
                        y: y_coord_val,
                        z: z_coord_val,
                    },
                };
                let location_name = location_handler::get_location_name_by_data(&loc);
                log::debug!(
                    "Item Non Event - Item is: {} ({:#X}) \nPTR: {:?}\n\
                X Coord: {} (X Addr: {:?})\n\
                Y Coord: {} (Y Addr: {:?})\n\
                Z Coord: {} (Z Addr: {:?})\n\
                Location Name: {:?}\n---
                ",
                    constants::get_item_name(item_id as u32),
                    item_id,
                    item_id_ptr,
                    x_coord_val,
                    x_coord_addr,
                    y_coord_val,
                    y_coord_addr,
                    z_coord_val,
                    z_coord_addr,
                    &location_name
                );
                match location_name {
                    Ok(location_name) => {
                        send_off_location_coords(
                            loc,
                            location_handler::get_mapped_item_id(location_name).unwrap(),
                        );
                    }
                    Err(err) => {
                        log::error!("{}", err);
                        if let Some(original) = ORIGINAL_HANDLE_PICKUP.get() {
                            original(item_struct);
                            return;
                        }
                    }
                }
            } else {
                const EXTRA_OUTPUT: bool = false;
                if EXTRA_OUTPUT {
                    log::error!(
                        "Item ID was above max ID: {:x} PTR: {:?}",
                        item_id,
                        item_id_ptr
                    );
                }
            }
        } else {
            // Special check for red orbs
            // ORIG_ID.store(0, SeqCst);
        }
        if let Some(original) = ORIGINAL_HANDLE_PICKUP.get() {
            original(item_struct);
        }
    }
}

fn is_valid_id(item_id: u32) -> bool {
    // Various orbs that I don't care about
    const INVALID_IDS: [u32; 13] = [
        // 0x05/0x06 are gold/yellow orbs
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    ];
    !INVALID_IDS.contains(&item_id)
}

/// Hook into item picked up method (1aa6e0). Handles item pick up locations
pub fn item_event(loc_chk_flg: usize, item_id: i16, unknown: i32) {
    unsafe {
        if is_valid_id(item_id as u32) && unknown == -1 {
            let mut loc = Location {
                location_type: LocationType::Standard,
                item_id: item_id as u32,
                room: game_manager::get_room(),
                mission: get_mission(),
                coordinates: EMPTY_COORDINATES,
            };
            const EXTRA_OUTPUT: bool = true;
            if EXTRA_OUTPUT && !EXTRA_EXTRA_OUTPUT {
                log::debug!(
                    "Item Event - Item is: {} ({:#X}) - LOC_CHK_FLG: {:X} - Unknown: {:X}",
                    constants::get_item_name(item_id as u32),
                    item_id,
                    loc_chk_flg,
                    unknown,
                );
            }
            // We only want items given via events, looks like if unknown is -1 then it'll always be an event item
            //log::debug!("Test: {:?}", location_handler::get_location_name_by_data(&loc))
            let location_name = location_handler::get_location_name_by_data(&loc);
            match location_name {
                Ok(location_name) => {
                    loc.item_id = generated_locations::ITEM_MISSION_MAP
                        .get(location_name)
                        .unwrap()
                        .item_id;
                    send_off_location_coords(
                        loc,
                        location_handler::get_mapped_item_id(location_name).unwrap(),
                    );
                }
                Err(err) => {
                    log::error!("Couldn't find location (Event): {}", err);
                    with_session_read(|s| {
                        log::debug!("Session Info: Mission: {} - Room: {}", s.mission, s.room);
                    })
                    .unwrap();
                }
            }
        }

        const EXTRA_EXTRA_OUTPUT: bool = false;
        if EXTRA_EXTRA_OUTPUT {
            log::debug!(
                "Item Event - Item is: {} ({:#X}) - LOC_CHK_FLG: {:X} - Unknown: {:X}",
                constants::get_item_name(item_id as u32),
                item_id,
                loc_chk_flg,
                unknown,
            );
        }
        if let Some(original) = ORIGINAL_ITEM_PICKED_UP.get() {
            original(loc_chk_flg, item_id, unknown);
        }
    }
}

/// To check off a mission as being completed
pub fn mission_complete_check(cuid_result: usize, ranking: i32) -> i32 {
    let final_ranking = if let Some(original) = ORIGINAL_RESULT_CALC.get() {
        unsafe { original(cuid_result, ranking) }
    } else {
        panic!("Result Calc doesn't exist??");
    };
    with_session_read(|s| {
        let rank = Rank::from_repr(final_ranking as usize).unwrap();
        let difficulty = if s.hoh {
            Difficulty::HeavenOrHell
        } else {
            Difficulty::from_repr(s.difficulty as usize).unwrap()
        };
        log::info!(
            "Mission {} Finished on Difficulty {} Rank {} ({})",
            s.mission,
            difficulty,
            rank, // If rank is 5 then SSS
            final_ranking
        );
        if let Some(mapping) = MAPPING.read().unwrap().as_ref() {
            // For SS Rank specific checks

            if rank == Rank::SS && !mapping.check_ss_difficulty
                || (mapping.check_ss_difficulty && difficulty >= mapping.mission_clear_difficulty)
            {
                send_off_location_coords(
                    Location {
                        location_type: LocationType::SSRank,
                        item_id: u32::MAX,
                        room: 0,
                        mission: s.mission,
                        coordinates: EMPTY_COORDINATES,
                    },
                    u32::MAX,
                );
            }

            // Minimum rank and difficulty
            if rank >= mapping.mission_clear_rank && difficulty >= mapping.mission_clear_difficulty
            {
                send_off_location_coords(
                    Location {
                        location_type: LocationType::MissionComplete,
                        item_id: u32::MAX,
                        room: 0,
                        mission: s.mission,
                        coordinates: EMPTY_COORDINATES,
                    },
                    u32::MAX,
                );
            }
        }
    })
    .expect("Session Data was not available?");
    final_ranking
}

pub const PURCHASE_ITEM_ADDR: usize = 0x285bb0;
pub static ORIGINAL_PURCHASE_ITEM: OnceLock<unsafe extern "C" fn(custom_gun: usize)> =
    OnceLock::new();
pub fn purchase_item_check(ptr: usize) {
    // Run original code, need consumables to still work
    if let Some(orig) = ORIGINAL_PURCHASE_ITEM.get() {
        unsafe {
            orig(ptr);
        }
    }

    if let Some(mapping) = MAPPING.read().unwrap().as_ref()
        && mapping.shop_checks
    {
        // Figure out the index of the item we just bought from the store.
        // 0xC8F263 is to check whether we are on gold or yellow
        let comb = ((read_data_from_address::<u8>(*DMC3_ADDRESS + 0xC8F263) as usize) * 7)
            + (read_data_from_address::<i32>(ptr + 0x419C) as usize);
        let shop_index = read_data_from_address::<i32>(*DMC3_ADDRESS + (comb * 4) + 0x00597688);
        if shop_index == 4 || shop_index == 5 {
            // I only need two of these, but may as well map them all out
            let bought_item_id = match shop_index {
                0 => 0x11, // Vital Star S
                1 => 0x10, // Vital Star L
                2 => 0x12, // Devil Star
                3 => 0x13, // Holy Water
                4 => 0x07, // Blue Orb
                5 => 0x08, // Purple Orb
                // Mode dependent (Only one of these will be visible)
                6 => 0x05, // Gold Orb
                7 => 0x06, // Yellow Orb
                _ => unreachable!(),
            };
            // Determine how many of these we have bought
            let amt = read_data_from_address::<u8>(
                read_data_from_address::<usize>(ptr + 0x4190) + 0xC + bought_item_id,
            );
            log::debug!("Item {} Bought {}", bought_item_id, amt);
            // Now that we know what was bought and how many times. We need to send this off to AP
            send_off_location_coords(Location {
                location_type: LocationType::PurchaseItem,
                item_id: bought_item_id as u32,
                mission: amt as u32,
                room: 0,
                coordinates: EMPTY_COORDINATES
            }, u32::MAX);
        }
    }
}

pub(crate) static TX_LOCATION: OnceLock<Sender<Location>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LocationType {
    Standard,
    MissionComplete,
    SSRank,
    PurchaseItem
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Location {
    pub(crate) location_type: LocationType,
    pub(crate) item_id: u32,
    pub(crate) room: i32,
    pub(crate) mission: u32,
    pub coordinates: Coordinates,
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Room ID: {:#} Item ID: {:#x}", self.room, self.item_id)
    }
}

impl PartialEq for Location {
    fn eq(&self, other: &Self) -> bool {
        self.coordinates == other.coordinates
            && self.room == other.room
            && self.item_id == other.item_id
    }
}

pub(crate) fn clear_high_roller() {
    log::debug!("Resetting high roller card");
    set_item("Remote", false, true);
    log::debug!("Resetting bomb");
    set_item("Dummy", false, true);
}

fn send_off_location_coords(loc: Location, to_display: u32) {
    if let Some(tx) = TX_LOCATION.get() {
        tx.send(loc).expect("Failed to send Location!");
        if to_display != u32::MAX {
            clear_high_roller();
            text_handler::LAST_OBTAINED_ID.store(to_display as u8, SeqCst);
        }
    }
}

pub(crate) fn take_away_received_item(id: u32) {
    if let Some(current_inv_addr) = get_inv_address() {
        let offset = *constants::ITEM_OFFSET_MAP
            .get(constants::ITEM_MAP.get_by_right(&id).unwrap())
            .unwrap_or_else(|| panic!("Item offset not found: {}", id));
        log::debug!("Stripping ID: {:#X} - Offset: {:#X}", id, offset);
        unsafe {
            randomizer_utilities::replace_single_byte(
                current_inv_addr + offset as usize,
                read_data_from_address::<u8>(current_inv_addr + offset as usize)
                    .saturating_sub(1),
            );
        }
    }
}
