pub(crate) use randomizer_utilities::{get_base_address, read_data_from_address};
use std::slice;
use std::sync::LazyLock;

/// The base address for DMC3
pub static DMC3_ADDRESS: LazyLock<usize> = LazyLock::new(|| get_base_address("dmc3.exe"));

pub(crate) fn is_on_main_menu() -> bool {
    // Seems to sometimes flicker to true when loading? At least when I went to the save selection screen
    read_data_from_address(*DMC3_ADDRESS + 0x5D9213)
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
