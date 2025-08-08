use crate::constants::{Rank, ITEM_OFFSET_MAP, ORIGINAL_HANDLE_PICKUP, ORIGINAL_ITEM_PICKED_UP, ORIGINAL_RESULT_CALC};
use crate::utilities::get_mission;
use crate::{archipelago, constants, data, utilities};
use data::generated_locations;
use once_cell::sync::OnceCell;
use std::ffi::c_int;
use std::fmt::{Display, Formatter};
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::SeqCst;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

static ORIG_ID: AtomicU8 = AtomicU8::new(0);

/// Hook into item handle method (1b45a0). Handles non-event item pick up locations
pub fn item_non_event(item_struct: i64) {
    unsafe {
        let base_ptr = item_struct as *const u8;
        let item_id_ptr = base_ptr.add(0x60) as *const i32; // Don't remove this
        let item_id = *(base_ptr.add(0x60));
        if item_id > 0x04 {
            // Ignore red orbs
            if item_id < 0x3A {
                let x_coord_addr = item_id_ptr.offset(0x1);
                let y_coord_addr = item_id_ptr.offset(0x2);
                let z_coord_addr = item_id_ptr.offset(0x3);
                let x_coord_val = (*(x_coord_addr) as u32).to_be();
                let y_coord_val = (*(y_coord_addr) as u32).to_be();
                let z_coord_val = (*(z_coord_addr) as u32).to_be();
                let loc = Location {
                    item_id: item_id as u64,
                    room: utilities::get_room(),
                    _mission: get_mission(),
                    x_coord: x_coord_val,
                    y_coord: y_coord_val,
                    z_coord: z_coord_val,
                };
                send_off_location_coords(loc.clone());
                let location_name = archipelago::get_location_item_name(&loc);
                log::debug!(
                    "Item Non Event - Item is: {} ({:#X}) PTR: {:?}\n\
                X Coord: {} (X Addr: {:?})\n\
                Y Coord: {} (Y Addr: {:?})\n\
                Z Coord: {} (Z Addr: {:?})\n\
                Location Name: {:?}\n---
                ",
                    constants::get_item_name(item_id),
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
                if location_name.is_err() {
                    if let Some(original) = ORIGINAL_HANDLE_PICKUP.get() {
                        original(item_struct);
                        return;
                    }
                }

                ORIG_ID.store(
                    generated_locations::ITEM_MISSION_MAP
                        .get(&location_name.unwrap())
                        .unwrap()
                        .item_id,
                    SeqCst,
                );
                //utilities::replace_single_byte_no_offset(item_id_ptr.addr(), generated_locations::ITEM_MISSION_MAP.get(location_name).unwrap().item_id)
            } else {
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
            ORIG_ID.store(0, SeqCst);
        }
        if let Some(original) = ORIGINAL_HANDLE_PICKUP.get() {
            original(item_struct);
        }
    }
}

const EXTRA_OUTPUT: bool = false;

/// Hook into item picked up method (1aa6e0). Handles item pick up locations
pub fn item_event(loc_chk_flg: i64, item_id: i16, unknown: i32) {
    unsafe {
        //utilities::replace_single_byte_no_offset(item_id, 0x11); // Just a little silly
        if item_id > 0x03 {
            if unknown == -1 {
                // We only want items given via events, looks like if unknown is -1 then it'll always be an event item
                send_off_location(item_id as i32);
            }
        }
        let mut item_id_orig = ORIG_ID.load(SeqCst) as i16;
        if item_id_orig == 0 {
            item_id_orig = item_id;
        }
        // log::debug!("Orig ID is: {:x}", item_id_orig);
        // log::debug!("Unknown is: {}", (20000 + item_id_orig) as c_int);
        if EXTRA_OUTPUT {
            log::debug!(
                "Item Event - Item is: {} ({:#X}) - LOC_CHK_FLG: {:X} - Unknown: {:X}",
                constants::get_item_name(item_id as u8),
                item_id,
                loc_chk_flg,
                unknown,
            );
        }
        if let Some(original) = ORIGINAL_ITEM_PICKED_UP.get() {
            original(loc_chk_flg, item_id_orig, (20000 + item_id_orig) as c_int);
            //original(loc_chk_flg, item_id, unknown);
        }
    }
}

/// To check off a mission as being completed
pub fn mission_complete_check(cuid_result: usize, ranking: i32) -> i32 {
    log::info!(
        "Mission {} Finished on Difficulty {} Rank {} ({})",
        get_mission(),
        utilities::get_difficulty(),
        Rank::from_repr(ranking as usize).unwrap(),
        ranking
    );
    if let Some(original) = ORIGINAL_RESULT_CALC.get() {
        unsafe {
            original(cuid_result, ranking)
        }
    } else {
        log::error!("Result Calc doesn't exist??");
        0
    }
}

pub(crate) static LOCATION_CHECK_TX: OnceCell<Sender<Location>> = OnceCell::new();

#[derive(Debug, Clone, Copy)]
pub(crate) struct Location {
    pub(crate) item_id: u64,
    pub(crate) room: i32,
    pub(crate) _mission: u8,
    pub(crate) x_coord: u32,
    pub(crate) y_coord: u32,
    pub(crate) z_coord: u32,
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(
            write!(f, "Room ID: {:#} Item ID: {:#x}", self.room, self.item_id)
                .expect("Failed to print Location as String!"),
        )
    }
}

pub(crate) fn setup_items_channel() -> Receiver<Location> {
    let (tx, rx) = mpsc::channel(32);
    LOCATION_CHECK_TX.set(tx).expect("TX already initialized");
    rx
}

pub(crate) fn clear_high_roller() {
    let current_inv_addr = utilities::get_inv_address();
    if current_inv_addr.is_none() {
        return;
    }
    log::debug!("Resetting high roller card");
    let item_addr =
        current_inv_addr.unwrap() + ITEM_OFFSET_MAP.get("Remote").unwrap().clone() as usize;
    log::debug!(
        "Attempting to replace at address: {:#X} with flag {:#X}",
        item_addr,
        0x00
    );
    unsafe { utilities::replace_single_byte(item_addr, 0x00) };
    log::debug!("Resetting bomb");
    let item_addr =
        current_inv_addr.unwrap() + ITEM_OFFSET_MAP.get("Dummy").unwrap().clone() as usize;
    log::debug!(
        "Attempting to replace at address: {:#X} with flag {:#X}",
        item_addr,
        0x00
    );
    unsafe { utilities::replace_single_byte(item_addr, 0x00) };
}

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn send_off_location(item_id: i32) {
    if let Some(tx) = LOCATION_CHECK_TX.get() {
        tx.send(Location {
            item_id: item_id as u64,
            room: utilities::get_room(),
            _mission: get_mission(),
            x_coord: 0,
            y_coord: 0,
            z_coord: 0,
        })
            .await
            .expect("Failed to send Location!");
        clear_high_roller();
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn send_off_location_coords(loc: Location) {
    if let Some(tx) = LOCATION_CHECK_TX.get() {
        tx.send(loc).await.expect("Failed to send Location!");
        clear_high_roller();
    }
}
