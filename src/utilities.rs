use crate::constants::{Difficulty, INVENTORY_PTR};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{read_unaligned, write_unaligned};
use std::slice;
use std::sync::{LazyLock, RwLock};
use winapi::shared::minwindef::HINSTANCE;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::PAGE_EXECUTE_READWRITE;

/// The base address for DMC3
pub static DMC3_ADDRESS: LazyLock<RwLock<usize>> =
    LazyLock::new(|| RwLock::new(get_base_address("dmc3.exe")));

pub fn get_inv_address() -> Option<usize> {
    let val = read_data_from_address(*DMC3_ADDRESS.read().unwrap() + INVENTORY_PTR);
    if val == 0 { None } else { Some(val) }
}

pub static MARY_ADDRESS: LazyLock<RwLock<usize>> =
    LazyLock::new(|| RwLock::new(get_base_address("Mary.dll")));

pub(crate) fn read_data_from_address<T>(address: usize) -> T
where
    T: Copy,
{
    unsafe { *(address as *const T) }
}

/// Generic method to get the base address for the specified module, returns 0 if it doesn't exist
fn get_base_address(module_name: &str) -> usize {
    let wide_name: Vec<u16> = OsStr::new(&module_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let module_handle: HINSTANCE = GetModuleHandleW(wide_name.as_ptr());
        if !module_handle.is_null() {
            module_handle as *mut _ as usize
        } else {
            0
        }
    }
}

/// Checks to see if DDMK is loaded
pub fn is_ddmk_loaded() -> bool {
    is_library_loaded("Mary.dll")
}

/// Checks to see if Crimson is loaded
pub fn _is_crimson_loaded() -> bool {
    is_library_loaded("Crimson.dll")
}

pub fn _is_addon_mod_loaded() -> bool {
    is_ddmk_loaded() || _is_crimson_loaded()
}

pub fn is_library_loaded(name: &str) -> bool {
    let wide_name: Vec<u16> = OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let module_handle: HINSTANCE = GetModuleHandleW(wide_name.as_ptr());
        !module_handle.is_null()
    }
}

pub unsafe fn replace_single_byte_with_base_addr(offset: usize, new_value: u8) {
    unsafe { replace_single_byte(offset + *DMC3_ADDRESS.read().unwrap(), new_value) }
}

const LOG_BYTE_REPLACEMENTS: bool = false;

//noinspection RsConstantConditionIf
pub unsafe fn replace_single_byte(offset: usize, new_value: u8) {
    unsafe {
        let length = 1;
        let mut old_protect = 0;
        VirtualProtect(
            offset as *mut _,
            length,
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );
        slice::from_raw_parts_mut(offset as *mut u8, length)[0] = new_value;
        VirtualProtect(offset as *mut _, length, old_protect, &mut old_protect);
        if LOG_BYTE_REPLACEMENTS {
            log::debug!(
                "Modified byte at: Offset: {:X}, byte: {:X}",
                offset,
                new_value
            );
        }
    }
}

const GAME_SESSION_DATA: usize = 0xC8F250;

/// Get current mission
pub fn get_mission() -> u8 {
    read_data_from_address(*DMC3_ADDRESS.read().unwrap() + GAME_SESSION_DATA)
}

/// Get current room
pub fn get_room() -> i32 {
    read_data_from_address(*DMC3_ADDRESS.read().unwrap() + GAME_SESSION_DATA + 0x8)
}

pub(crate) fn get_difficulty() -> Difficulty {
    Difficulty::from(read_data_from_address(
        *DMC3_ADDRESS.read().unwrap() + GAME_SESSION_DATA + 0xC,
    )) // Constant
}

pub(crate) fn get_dt_status() -> bool {
    // This also needs the player to have 3 runes of DT available for use
    read_data_from_address(*DMC3_ADDRESS.read().unwrap() + GAME_SESSION_DATA + 0xD1)
}

pub(crate) unsafe fn give_dt() {
    unsafe {
        replace_single_byte(
            *DMC3_ADDRESS.read().unwrap() + GAME_SESSION_DATA + 0xD1,
            u8::from(0x01),
        ); // This requires a fresh mission load to kick in
        let char_data_ptr: usize =
            read_data_from_address(*DMC3_ADDRESS.read().unwrap() + ACTIVE_CHAR_DATA);
        replace_single_byte(char_data_ptr + 0x139, u8::from(0x01)); // This requires a fresh mission load to kick in
        //give_magic(constants::ONE_ORB * 3.0); // Rune is 1000 each
    }
}

const CHARACTER_DATA: usize = 0xC90E30;
pub(crate) const ACTIVE_CHAR_DATA: usize = 0xCF2548;

pub(crate) fn give_magic(magic_val: f32) {
    let base = *DMC3_ADDRESS.read().unwrap();
    unsafe {
        write_unaligned(
            (base + GAME_SESSION_DATA + 0xD8) as *mut f32,
            read_unaligned((base + GAME_SESSION_DATA + 0xD8) as *mut f32) + magic_val,
        );
        write_unaligned(
            (base + CHARACTER_DATA + 0x16C + 0x6C) as *mut f32,
            read_unaligned((base + CHARACTER_DATA + 0x16C + 0x6C) as *mut f32) + magic_val,
        ); // Magic
        write_unaligned(
            (base + CHARACTER_DATA + 0x16C + 0x70) as *mut f32,
            read_unaligned((base + CHARACTER_DATA + 0x16C + 0x70) as *mut f32) + magic_val,
        ); // Max magic
        let char_data_ptr: usize =
            read_data_from_address(*DMC3_ADDRESS.read().unwrap() + ACTIVE_CHAR_DATA);
        write_unaligned(
            (char_data_ptr + 0x3EB8) as *mut f32,
            read_unaligned((char_data_ptr + 0x3EB8) as *mut f32) + magic_val,
        ); // Magic char
        write_unaligned(
            (char_data_ptr + 0x3EBC) as *mut f32,
            read_unaligned((char_data_ptr + 0x3EBC) as *mut f32) + magic_val,
        ); // Max magic char
    }
}

pub(crate) fn give_hp(life_value: f32) {
    let base = *DMC3_ADDRESS.read().unwrap();
    unsafe {
        write_unaligned(
            (base + CHARACTER_DATA + 0x16C + 0x64) as *mut f32,
            read_unaligned((base + CHARACTER_DATA + 0x16C + 0x64) as *mut f32) + life_value,
        ); // Life
        write_unaligned(
            (base + CHARACTER_DATA + 0x16C + 0x68) as *mut f32,
            read_unaligned((base + CHARACTER_DATA + 0x16C + 0x68) as *mut f32) + life_value,
        ); // Max life
        let char_data_ptr: usize =
            read_data_from_address(*DMC3_ADDRESS.read().unwrap() + ACTIVE_CHAR_DATA);
        write_unaligned(
            (char_data_ptr + 0x411C) as *mut f32,
            read_unaligned((char_data_ptr + 0x411C) as *mut f32) + life_value,
        ); // Life char
        write_unaligned(
            (char_data_ptr + 0x40EC) as *mut f32,
            read_unaligned((char_data_ptr + 0x40EC) as *mut f32) + life_value,
        ); // Max Life char
    }
}

pub(crate) fn set_max_hp(max_hp: f32) {
    unsafe {
        write_unaligned(
            (*DMC3_ADDRESS.read().unwrap() + GAME_SESSION_DATA + 0xD4) as *mut f32,
            max_hp,
        );
    }
}

pub(crate) fn set_max_magic(max_magic: f32) {
    unsafe {
        write_unaligned(
            (*DMC3_ADDRESS.read().unwrap() + GAME_SESSION_DATA + 0xD8) as *mut f32,
            max_magic,
        );
    }
}
