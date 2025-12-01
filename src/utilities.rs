use std::sync::LazyLock;
use std::{slice};
pub(crate) use randomizer_utilities::{get_base_address, read_data_from_address};

/// The base address for DMC3
pub static DMC3_ADDRESS: LazyLock<usize> = LazyLock::new(|| get_base_address("dmc3.exe"));

// Seems to sometimes flicker to true when loading? At least when I went to the save selection screen
pub fn is_on_main_menu() -> bool {
    read_data_from_address(*DMC3_ADDRESS + 0x5D9213)
}

pub fn get_inv_address() -> Option<usize> {
    const INVENTORY_PTR: usize = 0xC90E28 + 0x8;
    let val = read_data_from_address(*DMC3_ADDRESS + INVENTORY_PTR);
    if val == 0 { None } else { Some(val) }
}

pub fn get_active_char_address() -> Option<usize> {
    const ACTIVE_CHAR_PTR: usize = 0xCF2548;
    let val = read_data_from_address(*DMC3_ADDRESS + ACTIVE_CHAR_PTR);
    if val == 0 { None } else { Some(val) }
}

pub fn get_event_address() -> Option<usize> {
    // Remember kids, assuming makes an ass out of u and ming
    const EVENT_PTR: usize = 0xC9DDB8;
    let event_table_addr: usize = read_data_from_address::<usize>(*DMC3_ADDRESS + EVENT_PTR);

    if unsafe { slice::from_raw_parts(event_table_addr as *const u8, 3) } != b"EVT" {
        log::error!("Pointer was not pointing to event table");
        return None;
    }
    Some(event_table_addr)
}

/// Checks to see if DDMK is loaded
pub fn is_ddmk_loaded() -> bool {
    randomizer_utilities::is_library_loaded("Mary.dll")
}

/// Checks to see if Crimson is loaded
pub fn is_crimson_loaded() -> bool {
    randomizer_utilities::is_library_loaded("Crimson.dll")
}

pub fn _is_addon_mod_loaded() -> bool {
    is_ddmk_loaded() || is_crimson_loaded()
}

pub unsafe fn replace_single_byte_with_base_addr(offset: usize, new_value: u8) {
    unsafe { randomizer_utilities::replace_single_byte(offset + *DMC3_ADDRESS, new_value) }
}

