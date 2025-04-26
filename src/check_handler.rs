use once_cell::sync::OnceCell;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::fmt::{Display, Formatter};
use crate::constants::ORIGINAL_ITEMPICKEDUP;
use crate::{constants, utilities};
use crate::utilities::get_mission;

/// Hook into item picked up method (1aa6e0). Handles item pick up locations
pub fn item_picked_up_hook(loc_chk_flg: i64, item_id: i16, unknown: i32) { unsafe {
    if item_id > 0x03 {
        log::debug!("Loc CHK Flg is: {:x}", loc_chk_flg);
        log::debug!("Item ID is: {} (0x{:x})", constants::get_item(item_id as u64), item_id);
        log::debug!("Unknown is: {:x}", unknown); // Don't know what to make of this just yet
        let mut room_5 = false;
        if unknown == 10013 {
            room_5 = true; // Adjudicator
        }

        if let Some(tx) = LOCATION_CHECK_TX.get() {
            tx.send(Location {
                item_id: item_id as u64,
                room: utilities::get_room(),
                mission: get_mission(), // About to add a fucking flag for room 5
                room_5
            })
                .expect("Failed to send Location!");
        }
    }


    if let Some(original) = ORIGINAL_ITEMPICKEDUP {
        original(loc_chk_flg, item_id, unknown);
    }
}}

/// To check off a mission as being completed - TODO
pub fn mission_complete_check() {
    if let Some(tx) = LOCATION_CHECK_TX.get() {
        tx.send(Location {
            item_id: 0,
            room: 0,
            mission: get_mission(),
            room_5: false
        })
            .expect("Failed to send Location!");
    }
}
 
pub(crate) static LOCATION_CHECK_TX: OnceCell<Sender<Location>> = OnceCell::new();

pub(crate) struct Location {
    pub(crate) item_id: u64,
    pub(crate) room: i32,
    pub(crate) mission: i32,
    pub(crate) room_5: bool // This room is evil due to the adjudicator and visible blue frag, TODO Cut?
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