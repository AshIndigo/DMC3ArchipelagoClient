use crate::DMC3_ADDRESS;
use crate::MinHook;
use crate::game_manager::get_mission;
use crate::{AP_CORE, create_hook};
use archipelago_rs::{AsLocationId, Location};
use minhook::MH_STATUS;
use oneshot::Receiver;
use rand::seq::IteratorRandom;
use randomizer_utilities::read_data_from_address;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{LazyLock, OnceLock};
use std::thread;

pub static TX_HINT: OnceLock<Sender<Vec<i64>>> = OnceLock::new();
pub static FLOORS_PER_HINT: AtomicU16 = AtomicU16::new(0);

pub unsafe fn create_hint_hooks() -> Result<(), MH_STATUS> {
    unsafe {
        create_hook!(
            EVENT_HANDLER_ADDR,
            on_bp_floor_change,
            ORIGINAL_EVENT_HANDLER,
            "Monitor BP Floor"
        );
    }
    Ok(())
}

static HINT_HOOK_ADDRESSES: LazyLock<Vec<usize>> = LazyLock::new(|| {
    const ADDRESSES: [usize; 1] = [EVENT_HANDLER_ADDR];
    ADDRESSES.to_vec()
});

pub(crate) unsafe fn enable_hint_hooks() {
    HINT_HOOK_ADDRESSES.iter().for_each(|addr| unsafe {
        if let Err(err) = MinHook::enable_hook((*DMC3_ADDRESS + addr) as *mut _) {
            log::error!("Failed to enable {:#X} hook: {:?}", addr, err);
        }
    })
}

pub fn disable_hint_hooks() -> Result<(), MH_STATUS> {
    let base_address = *DMC3_ADDRESS;
    HINT_HOOK_ADDRESSES.iter().for_each(|addr| unsafe {
        if let Err(err) = MinHook::disable_hook((base_address + *addr) as *mut _) {
            log::error!("Failed to disable {:#X} hook: {:?}", addr, err);
        }
    });
    Ok(())
}

const EVENT_HANDLER_ADDR: usize = 0x1A6510;
pub static ORIGINAL_EVENT_HANDLER: OnceLock<
    unsafe extern "C" fn(param_1: usize, event_code: usize) -> i32, // u8
> = OnceLock::new();

unsafe fn read_ptr<T>(p: *const *const T) -> Option<*const T> {
    let v = unsafe { p.read() };
    if v.is_null() { None } else { Some(v) }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct BloodyPalaceData {
    unknown: [u8; 2084],
    level: u16,
    last_level: u16,
}

unsafe fn get_bloody_palace_data() -> Option<&'static BloodyPalaceData> {
    static BP_DATA: LazyLock<usize> = LazyLock::new(|| *DMC3_ADDRESS + 0xC90E10);
    unsafe {
        let addr1 = read_ptr(*BP_DATA as *const *const *const u8)?;
        let addr2 = read_ptr(addr1.add(5))?;
        let addr3 = read_ptr(addr2.add(0x10) as *const *const u8)?;
        let addr4 = read_ptr(addr3.add(8) as *const *const u8)?;
        Some(&*(addr4 as *const BloodyPalaceData))
    }
}

fn on_bp_floor_change(param_1: usize, event_data_ptr: usize) -> i32 {
    let mut res = 0;
    unsafe {
        if let Some(original) = ORIGINAL_EVENT_HANDLER.get() {
            res = original(param_1, event_data_ptr)
        }
    }
    const BLOODY_PALACE: u32 = 21;
    if get_mission() == BLOODY_PALACE {
        unsafe {
            // Get event code to see if we're adding to the BP floor
            if read_data_from_address::<u8>(read_data_from_address::<usize>(event_data_ptr)) == 0x8A
                && let Some(bp_data) = get_bloody_palace_data()
                && let Some(client) = AP_CORE
                    .get()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .connection
                    .client_mut()
            {
                let floors_per_hint: u16 = FLOORS_PER_HINT.load(Ordering::SeqCst);
                let key = format!(
                    "_read_hints_{}_{}",
                    client.this_player().team(),
                    client.this_player().slot()
                );
                fire_off_hints(
                    client.get([key.clone()]),
                    client.unchecked_locations().collect(),
                    (bp_data.last_level / floors_per_hint + 1) * floors_per_hint,
                    (bp_data.level / floors_per_hint) * floors_per_hint,
                    floors_per_hint,
                    key,
                )
            }
        }
    }
    res
}

#[derive(Deserialize, Debug)]
struct Hint {
    #[serde(rename = "receiving_player")]
    _receiving_player: u32,
    #[serde(rename = "finding_player")]
    _finding_player: u32,
    location: i64,
    #[serde(rename = "item")]
    _item: i64,
    #[serde(rename = "found")]
    _found: bool,
    #[serde(rename = "entrance")]
    _entrance: String,
    #[serde(rename = "item_flags")]
    _item_flags: i32,
    #[serde(rename = "status")]
    _status: i32, // HintStatus
}

fn fire_off_hints(
    future: Receiver<Result<HashMap<String, serde_json::Value>, archipelago_rs::Error>>,
    unchecked_locations: Vec<Location>,
    mut f: u16,
    last: u16,
    floors_per_hint: u16,
    key: String,
) {
    // All on a separate thread as to not lock up the main one
    thread::spawn(move || match future.recv() {
        Ok(res) => match res {
            Ok(locations) => {
                // List of already hinted locations
                let hinted_locations: Vec<i64> =
                    Vec::<Hint>::deserialize(locations.get(&key).unwrap())
                        .unwrap()
                        .iter_mut()
                        .map(|hint| hint.location)
                        .collect();

                // Figure out valid locations to send out a hint for
                let mut hints_to_make = vec![];
                // Possible locations to send hints for
                // Made by checking to see which unchecked locations don't already have hints
                let mut possible_locations = unchecked_locations
                    .into_iter()
                    .filter(|loc| !hinted_locations.contains(&loc.as_location_id()))
                    .map(|loc| loc.as_location_id())
                    .collect::<Vec<i64>>();
                // RNG!
                let rng = &mut rand::rng();
                // Loop for determining how many hints we need
                while f <= last {
                    // For each hint we need to make, pick a random location and remove it from the pool
                    hints_to_make.push(
                        possible_locations.swap_remove(
                            (0..possible_locations.len())
                                .choose(rng)
                                .unwrap_or_default(),
                        ),
                    );
                    f += floors_per_hint;
                }
                // Send off on a channel, main thread will send run CreateHints
                TX_HINT.get().unwrap().send(hints_to_make).unwrap();
            }
            Err(err) => {
                log::error!("Failed to get hinted locations: {:?}", err);
            }
        },
        Err(err) => {
            log::error!("Failed to receive hinted locations: {:?}", err);
        }
    });
}
