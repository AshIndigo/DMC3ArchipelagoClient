use crate::constants::{Coordinates, Difficulty, Rank, EMPTY_COORDINATES};
use crate::game_manager::{get_mission, set_item, with_session_read};
use crate::utilities::{get_inv_address, DMC3_ADDRESS};
use crate::{constants, create_hook, game_manager, location_handler, text_handler, utilities};
use minhook::{MinHook, MH_STATUS};
use once_cell::sync::OnceCell;
use std::fmt::{Display, Formatter};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::OnceLock;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

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
    }
    Ok(())
}

pub unsafe fn disable_check_hooks(base_address: usize) -> Result<(), MH_STATUS> {
    log::debug!("Disabling check related hooks");
    unsafe {
        MinHook::disable_hook((base_address + ITEM_HANDLE_PICKUP_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + ITEM_PICKED_UP_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + RESULT_CALC_ADDR) as *mut _)?;
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
                    item_id: item_id as u32,
                    room: game_manager::get_room(),
                    _mission: get_mission(),
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
                            loc.clone(),
                            location_handler::get_mapped_item_id(&location_name).unwrap(), //location_handler::get_item_at_location(&loc).unwrap(),
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
    const INVALID_IDS: [u32; 11] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    ];
    !INVALID_IDS.contains(&item_id)
}

/// Hook into item picked up method (1aa6e0). Handles item pick up locations
pub fn item_event(loc_chk_flg: usize, item_id: i16, unknown: i32) {
    unsafe {
        if is_valid_id(item_id as u32) {
            if unknown == -1 {
                let loc = Location {
                    item_id: item_id as u32,
                    room: game_manager::get_room(),
                    _mission: get_mission(),
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
    with_session_read(|s| {
        log::info!(
            "Mission {} Finished on Difficulty {} Rank {} ({})",
            s.mission,
            Difficulty::from_repr(s.difficulty as usize).unwrap(),
            Rank::from_repr(ranking as usize).unwrap(), // If rank is 5 then SSS
            ranking
        );
        if s.mission == 20 {
            send_off_location_coords(M20, u32::MAX);
        }
    })
    .expect("Session Data was not available?");
    if let Some(original) = ORIGINAL_RESULT_CALC.get() {
        unsafe { original(cuid_result, ranking) }
    } else {
        panic!("Result Calc doesn't exist??");
    }
}

pub(crate) static LOCATION_CHECK_TX: OnceCell<Sender<Location>> = OnceCell::new();

#[derive(Debug, Clone, Copy)]
pub(crate) struct Location {
    pub(crate) item_id: u32,
    pub(crate) room: i32,
    pub(crate) _mission: u32,
    pub coordinates: Coordinates,
}

pub const M20: Location = Location {
    item_id: u32::MAX,
    room: -1,
    _mission: 20,
    coordinates: EMPTY_COORDINATES,
};

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(
            write!(f, "Room ID: {:#} Item ID: {:#x}", self.room, self.item_id)
                .expect("Failed to print Location as String!"),
        )
    }
}

impl PartialEq for Location {
    fn eq(&self, other: &Self) -> bool {
        self.coordinates == other.coordinates
            && self.room == other.room
            && self.item_id == other.item_id
    }
}

pub(crate) fn setup_items_channel() -> Receiver<Location> {
    let (tx, rx) = mpsc::channel(32);
    LOCATION_CHECK_TX.set(tx).expect("TX already initialized");
    rx
}

pub(crate) fn clear_high_roller() {
    log::debug!("Resetting high roller card");
    set_item("Remote", false, true);
    log::debug!("Resetting bomb");
    set_item("Dummy", false, true);
}

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn send_off_location_coords(loc: Location, to_display: u32) {
    if let Some(tx) = LOCATION_CHECK_TX.get() {
        tx.send(loc).await.expect("Failed to send Location!");
        if to_display != u32::MAX {
            clear_high_roller();
            text_handler::LAST_OBTAINED_ID.store(to_display as u8, SeqCst);
            take_away_received_item(loc.item_id);
        }
    }
}

fn take_away_received_item(id: u32) {
    if let Some(current_inv_addr) = get_inv_address() {
        let offset = *constants::ITEM_OFFSET_MAP
            .get(constants::ID_ITEM_MAP.get(&id).unwrap())
            .unwrap_or_else(|| panic!("Item offset not found: {}", id));
        log::debug!("Offset: {}", offset); // Using remote rather than actual id
        unsafe {
            utilities::replace_single_byte(
                current_inv_addr + offset as usize,
                utilities::read_data_from_address::<u8>(current_inv_addr + offset as usize)
                    .saturating_sub(1),
            );
        }
    }
}
