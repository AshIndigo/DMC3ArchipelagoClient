use crate::mapping::MAPPING;
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
use crate::connection_manager::CONNECTION_STATUS;

/// Pointer to where save file is in memory
const SAVE_FILE_PTR: usize = 0x5EAE78;
pub const SAVE_GAME_ADDR: usize = 0x3a6e0;
pub static ORIGINAL_SAVE_GAME: OnceLock<unsafe extern "C" fn(param_1: i32)> = OnceLock::new();

pub const LOAD_GAME_ADDR: usize = 0x3a5e0;
pub static ORIGINAL_LOAD_GAME: OnceLock<
    unsafe extern "C" fn(param_1: i64, param_2: i64, save_data_ptr: usize, length: i32) -> i32,
> = OnceLock::new();

static SAVE_DATA: RwLock<Vec<u8>> = RwLock::new(vec![]);

pub fn get_save_path() -> Result<String, io::Error> {
    // Load up the mappings to get the seed
    if let Some(mappings) = MAPPING.read().unwrap().as_ref() {
        Ok(format!("archipelago/dmc3_{}.sav", &mappings.seed))
    } else {
        Err(io::Error::other("Mappings not available"))
    }
}

pub fn get_new_save_path() -> Result<String, Box<dyn Error>> {
    // Load up the mappings to get the seed
    if let Some(mappings) = MAPPING.read()?.as_ref() {
        if let Ok(core) = AP_CORE.get().unwrap().as_ref().lock() {
            Ok(format!(
                "archipelago/dmc3_{}_{}.sav",
                &mappings.seed,
                core.connection.client().unwrap().this_player().name()
            ))
        } else {
            Err("Connecting unavailable".into())
        }
        
    } else {
        Err("Mappings not available".into())
    }
}

pub fn setup_save_hooks() -> Result<(), MH_STATUS> {
    log::debug!("Setting up save file related hooks");
    unsafe {
        create_hook!(
            LOAD_GAME_ADDR,
            new_load_game,
            ORIGINAL_LOAD_GAME,
            "Load game"
        );
        create_hook!(
            SAVE_GAME_ADDR,
            new_save_game,
            ORIGINAL_SAVE_GAME,
            "Save game"
        );
    }
    Ok(())
}

/// Reimplementation of DMC3's save game method, but will save to a custom file instead
fn new_save_game(param_1: i32) {
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
/// Triggers everytime the 10 save slots are displayed. Probably when the game is also first loaded to control Vergil access, but that shouldn't matter
fn new_load_game(param_1: i64, param_2: i64, save_data_ptr: *mut usize, length: i32) -> i32 {
    // Returns 1 (loaded successfully?) or -1 (failed for whatever reason)
    log::debug!("Loading save slot selection screen!");
    if CONNECTION_STATUS.load(Ordering::SeqCst) == 1 {
        return match get_save_data() {
            Ok(..) => {
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
    match fs::read(get_new_save_path()?) {
        Ok(bytes) => {
            *SAVE_DATA.write()? = bytes;
            Ok(())
        }
        Err(err) => match err.kind() {
            ErrorKind::NotFound => match fs::read(get_save_path()?) {
                Ok(bytes) => {
                    *SAVE_DATA.write()? = bytes;
                    Ok(())
                }
                Err(err) => Err(Box::new(err)),
            },
            _ => Err(Box::new(err)),
        },
    }
}
