use crate::constants::get_weapon_id;
use crate::mapping::MAPPING;
use crate::ui::ui;
use crate::utilities::DMC3_ADDRESS;
use crate::{create_hook, utilities};
use minhook::MinHook;
use minhook::MH_STATUS;
use std::fs;
use std::ptr::{write, write_unaligned};
use std::sync::atomic::Ordering;
use std::sync::{OnceLock, RwLock};
// When connected to room
// - Would want to only do this on the main menu

/// Pointer to where save file is in memory
const SAVE_FILE_PTR: usize = 0x5EAE78;
pub const SAVE_GAME_ADDR: usize = 0x3a6e0;
pub static ORIGINAL_SAVE_GAME: OnceLock<unsafe extern "C" fn(param_1: i32)> = OnceLock::new();

pub const LOAD_GAME_ADDR: usize = 0x3a5e0;
pub static ORIGINAL_LOAD_GAME: OnceLock<
    unsafe extern "C" fn(param_1: i64, param_2: i64, save_data_ptr: usize, length: i32) -> i32,
> = OnceLock::new();

static SAVE_DATA: RwLock<Vec<u8>> = RwLock::new(vec![]);

pub fn get_save_path() -> Result<String, Box<dyn std::error::Error>> {
    // Load up the mappings to get the seed
    let guard = MAPPING.read()?;
    let mappings = guard
        .as_ref()
        .ok_or("No mapping found, seed is unavailable")?;
    Ok(format!("archipelago/dmc3_{}.sav", &mappings.seed))
}
/* Default save
- All difficulties available
- All costumes except for super costumes
- Vergil is not unlocked
- Gold mode
- Tutorials off
- BP + Gallery + Demo Digest (Though contents are not unlocked)
- DT Unlocked
- Blue/Purple orbs made unpurchasable
*/
/*
Can modify this save file to have appropriate starter weapons immediately equipped
Additionally, maybe option on AP site to configure whether gold or yellow mode? Also want to somehow disable new game option?
*/
const SAVE_SIZE: usize = 0x4A30;
pub static ORIGINAL_WRITE_CRC: OnceLock<
    // Don't know first param
    // Second param is for the event code and then additional data. Might actually be i32's?
    unsafe extern "C" fn(param_1: usize, event_code: i32), // u8
> = OnceLock::new();

/// If the seed save does not exist, create it
pub(crate) fn create_special_save() -> Result<(), Box<dyn std::error::Error>> {
    const ORIGINAL_CRC_ADDR: usize = 0x33eed0;
    const BASE_BYTES: &[u8; SAVE_SIZE] = include_bytes!("data/dmc3_all.sav");
    let save_bytes = BASE_BYTES.clone();
    let save_path = get_save_path()?;
    if !fs::exists("archipelago")? {
        fs::create_dir("archipelago/")?;
    }
    let save_bytes = set_starting_weapons(save_bytes)?;
    unsafe {
        ORIGINAL_WRITE_CRC.get_or_init(|| std::mem::transmute(*DMC3_ADDRESS + ORIGINAL_CRC_ADDR))(
            save_bytes.as_ptr().addr() + 0x3B8,
            0x708,
        );
    }
    if !fs::exists(&save_path)? {
        fs::write(save_path, save_bytes)?;
    }
    Ok(())
}

fn set_starting_weapons(
    mut save_bytes: [u8; SAVE_SIZE],
) -> Result<[u8; SAVE_SIZE], Box<dyn std::error::Error>> {
    let guard = MAPPING.read()?;
    let mappings = guard
        .as_ref()
        .ok_or("No mapping found, starting weapons are unavailable")?;
    let mut melee = 0;
    let mut gun = 5;

    if !&mappings.start_melee.as_str().eq("Rebellion") {
        melee = get_weapon_id(&mappings.start_melee.as_str());
    }
    if !&mappings.start_gun.as_str().eq("Ebony & Ivory") {
        gun = get_weapon_id(&mappings.start_gun.as_str())
    }
    log::debug!("Gun ID: {:#X} - Melee ID: {:#X}", gun, melee);
    save_bytes[0x414] = melee;
    save_bytes[0x416] = gun;
    Ok(save_bytes)
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

pub unsafe fn disable_save_hooks(base_address: usize) -> Result<(), MH_STATUS> {
    log::debug!("Disabling save related hooks");
    unsafe {
        MinHook::disable_hook((base_address + LOAD_GAME_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + SAVE_GAME_ADDR) as *mut _)?;
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

            fs::write(get_save_path().expect("Unable to get save path"), data)
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
    if ui::CONNECTION_STATUS.load(Ordering::SeqCst) == 1 {
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
                log::error!("Error getting save data: {}", err);
                -1
            }
        };
    }
    if let Some(original) = ORIGINAL_LOAD_GAME.get() {
        unsafe {
            let return_val = original(param_1, param_2, *save_data_ptr, length);
            log::debug!("Return val: {}", return_val);
            return_val
        }
    } else {
        panic!("Original Load game address not found");
    }
}

/// Get the save data to store in the SAVE_DATA global
fn get_save_data() -> Result<(), Box<dyn std::error::Error>> {
    *SAVE_DATA.write().unwrap() = fs::read(get_save_path()?)?;
    Ok(())
}
