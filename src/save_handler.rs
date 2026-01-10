use crate::archipelago::CONNECTION_STATUS;
use crate::utilities::DMC3_ADDRESS;
use crate::{create_hook, utilities, AP_CORE};
use minhook::MinHook;
use minhook::MH_STATUS;
use std::error::Error;
use std::io::ErrorKind;
use std::ptr::{write, write_unaligned};
use std::sync::atomic::Ordering;
use std::sync::{OnceLock, RwLock};
use std::{fs, io};

/// Pointer to where save file is in memory
const SAVE_FILE_PTR: usize = 0x5EAE78;
pub const SAVE_GAME_ADDR: usize = 0x3a6e0;
pub static ORIGINAL_SAVE_GAME: OnceLock<unsafe extern "C" fn(param_1: i32)> = OnceLock::new();

pub const LOAD_GAME_ADDR: usize = 0x3a5e0;
pub static ORIGINAL_LOAD_GAME: OnceLock<
    unsafe extern "C" fn(param_1: i64, param_2: i64, save_data_ptr: usize, length: i32) -> i32,
> = OnceLock::new();

static SAVE_DATA: RwLock<Vec<u8>> = RwLock::new(vec![]);

pub fn get_new_save_path() -> Result<String, Box<dyn Error>> {
    if let Ok(core) = AP_CORE.get().unwrap().as_ref().lock() {
        let client = core.connection.client().unwrap();
        Ok(format!(
            "archipelago/dmc3_{}_{}.sav",
            client.seed_name(),
            client.this_player().name()
        ))
    } else {
        Err("Connecting unavailable".into())
    }
}

pub fn setup_save_hooks() -> Result<(), MH_STATUS> {
    log::debug!("Setting up save file related hooks");
    unsafe {
        create_hook!(
            LOAD_GAME_ADDR,
            load_ap_save_file,
            ORIGINAL_LOAD_GAME,
            "Load game"
        );
        create_hook!(
            SAVE_GAME_ADDR,
            save_ap_save_file,
            ORIGINAL_SAVE_GAME,
            "Save game"
        );
        create_hook!(
            SAVE_SESSION_DATA,
            save_to_slot,
            ORIGINAL_SAVE_SLOT,
            "Save slot"
        );
        create_hook!(
            LOAD_SESSION_DATA,
            load_slot,
            ORIGINAL_LOAD_SLOT,
            "Load slot"
        );
    }
    Ok(())
}

/// Reimplementation of DMC3's save game method, but will save to a custom file instead
fn save_ap_save_file(param_1: i32) {
    // param_1 has just been 0 so far
    log::debug!("Saving game (1) Param_1: {}", param_1);
    let base = *DMC3_ADDRESS;
    unsafe {
        if param_1 == 0 && (utilities::read_data_from_address::<u8>(base + 0x5EAE81) != 0) {
            let save_file_ptr = (base + SAVE_FILE_PTR) as *const usize;
            let save_file = save_file_ptr.read() as *const u8;
            let len_ptr = (base + 0x5eae74) as *const i32; // Save length address

            let len = len_ptr.read(); // AFAIK This is a constant value, but may as well get it from the game just to be safe
            let data = std::slice::from_raw_parts(save_file, len as usize).to_vec();

            fs::write(get_new_save_path().expect("Unable to get save path"), data)
                .expect("Unable to save game");
            utilities::replace_single_byte_with_base_addr(0x5EAE81, 0x0);
        }
        // Don't really know what this does, but it's probably important
        utilities::replace_single_byte_with_base_addr(0x560b78, 0x0);
        write_unaligned((base + 0x560b70) as *mut i32, 0);
        write_unaligned((base + 0x560b7c) as *mut i32, 10);
    }
    if let Some(original) = ORIGINAL_SAVE_GAME.get() {
        unsafe { original(param_1) }
    }
}

/// Hook for the games load game method
/// Triggers everytime the 10 save slots are displayed. As well as when the game is first loaded
fn load_ap_save_file(param_1: i64, param_2: i64, save_data_ptr: *mut usize, length: i32) -> i32 {
    // Returns 1 (loaded successfully?) or -1 (failed for whatever reason)
    log::debug!("Loading save slot selection screen!");
    if CONNECTION_STATUS.load(Ordering::SeqCst) == 1 {
        return match get_save_data() {
            Ok(_) => {
                unsafe {
                    write(
                        (*DMC3_ADDRESS + SAVE_FILE_PTR) as *mut usize,
                        SAVE_DATA.read().unwrap().as_ptr().addr(),
                    );
                }
                1
            }
            Err(err) => {
                match err.downcast::<io::Error>() {
                    Ok(err) => match err.kind() {
                        ErrorKind::NotFound => {}
                        _ => {
                            log::error!("Error getting save data: {}", err);
                        }
                    },
                    Err(failed) => {
                        log::error!("Error getting save data: {}", failed);
                    }
                }
                -1
            }
        };
    }
    if let Some(original) = ORIGINAL_LOAD_GAME.get() {
        unsafe { original(param_1, param_2, *save_data_ptr, length) }
    } else {
        panic!("Original Load game address not found");
    }
}

/// Get the save data to store in the SAVE_DATA global
fn get_save_data() -> Result<(), Box<dyn Error>> {
    *SAVE_DATA.write()? = fs::read(get_new_save_path()?)?;
    Ok(())
}

pub const LOAD_SESSION_DATA: usize = 0x3297E0;
pub static ORIGINAL_LOAD_SLOT: OnceLock<unsafe extern "C" fn(usize, i32)> = OnceLock::new();
fn load_slot(param_1: usize, save_index: i32) {
    if let Some(orig) = ORIGINAL_LOAD_SLOT.get() {
        unsafe {
            orig(param_1, save_index);
        }
    }
    log::debug!("Loading from slot: {}", save_index);
}

pub const SAVE_SESSION_DATA: usize = 0x32B080;
pub static ORIGINAL_SAVE_SLOT: OnceLock<unsafe extern "C" fn(usize, i32)> = OnceLock::new();
fn save_to_slot(param_1: usize, save_index: i32) {
    if let Some(orig) = ORIGINAL_SAVE_SLOT.get() {
        unsafe {
            orig(param_1, save_index);
        }
    }
    log::debug!("Saving to slot {}", save_index);
}
