use once_cell::sync::OnceCell;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::fmt::{Display, Formatter};
use crate::constants::{ORIGINAL_HANDLE_PICKUP, ORIGINAL_ITEM_PICKED_UP};
use crate::{constants, utilities};
use crate::utilities::get_mission;

/// Hook into item handle method (1b45a0). Handles non-event item pick up locations
pub fn item_non_event(item_struct: i64) { unsafe {
    let base_ptr = item_struct as *const u8;
    let item_id_ptr = base_ptr.add(0x60) as *const i32;
    let item_id = *item_id_ptr;
    if item_id > 0x03 {
        log::debug!("Item ID is: {} (0x{:x})", constants::get_item(item_id as u64), item_id);
        log::debug!("Item ID PTR is: {:?}", item_id_ptr);
        let x_coord = item_id_ptr.offset(0x1);
        let y_coord = item_id_ptr.offset(0x2);
        let z_coord = item_id_ptr.offset(0x3);
        let x_coord_val = (*(x_coord) as u32).to_be();
        let y_coord_val = (*(y_coord) as u32).to_be();
        let z_coord_val = (*(z_coord) as u32).to_be();
        log::debug!("X Addr: {:?}, X Coord: {}", x_coord, x_coord_val);
        log::debug!("Y Addr: {:?}, Y Coord: {}", y_coord, y_coord_val);
        log::debug!("Z Addr: {:?}, Z Coord: {}", z_coord, z_coord_val);
        send_off_location_coords(item_id, x_coord_val, y_coord_val, z_coord_val);
    }

    if let Some(original) = ORIGINAL_HANDLE_PICKUP {
        original(item_struct);
    }
}}

fn send_off_location(item_id: i32) {
    if let Some(tx) = LOCATION_CHECK_TX.get() {
        tx.send(Location {
            item_id: item_id as u64,
            room: utilities::get_room(),
            _mission: get_mission(), // About to add a fucking flag for room 5
            x_coord: 0,
            y_coord: 0,
            z_coord: 0,
        })
            .expect("Failed to send Location!");
    }
}

fn send_off_location_coords(item_id: i32, x_coord: u32, y_coord: u32, z_coord: u32) {
    if let Some(tx) = LOCATION_CHECK_TX.get() {
        tx.send(Location {
            item_id: item_id as u64,
            room: utilities::get_room(),
            _mission: get_mission(), // About to add a fucking flag for room 5
            x_coord,
            y_coord,
            z_coord,
        })
            .expect("Failed to send Location!");
    }
}

/// Hook into item picked up method (1aa6e0). Handles item pick up locations
pub fn item_event(loc_chk_flg: i64, item_id: i16, unknown: i32) { unsafe {
    if item_id > 0x03 {
        if unknown == -1 { // We only want items given via events, looks like if unknown is -1 then it'll always be an event item
            log::debug!("Loc CHK Flg is: {:x}", loc_chk_flg);
            log::debug!("Item ID is: {} (0x{:x})", constants::get_item(item_id as u64), item_id);
            log::debug!("Unknown is: {:x}", unknown); // Don't know what to make of this just yet
            send_off_location(item_id as i32);
        }
    }

    if let Some(original) = ORIGINAL_ITEM_PICKED_UP {
        original(loc_chk_flg, item_id, unknown);
    }
}}

/// To check off a mission as being completed - TODO
pub fn mission_complete_check() {
    if let Some(tx) = LOCATION_CHECK_TX.get() {
        tx.send(Location {
            item_id: 0,
            room: 0,
            _mission: get_mission(),
            x_coord: 0,
            y_coord: 0,
            z_coord: 0,
        })
            .expect("Failed to send Location!");
    }
}
 
pub(crate) static LOCATION_CHECK_TX: OnceCell<Sender<Location>> = OnceCell::new();

pub(crate) struct Location {
    pub(crate) item_id: u64,
    pub(crate) room: i32,
    pub(crate) _mission: i32,
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

pub(crate) fn setup_items_channel() -> Arc<Mutex<Receiver<Location>>> {
    let (tx, rx) = mpsc::channel();
    LOCATION_CHECK_TX.set(tx).expect("TX already initialized");
    Arc::new(Mutex::new(rx))
}