use crate::constants::INVENTORY_PTR;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::slice;
use std::sync::{LazyLock, RwLock};
use winapi::shared::minwindef::HINSTANCE;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::PAGE_EXECUTE_READWRITE;

/// The base address for DMC3
pub static DMC3_ADDRESS: LazyLock<RwLock<usize>> =
    LazyLock::new(|| RwLock::new(get_base_address("dmc3.exe")));

pub fn get_inv_address() -> usize {
    // TODO Option, Return None if it's 0?
    read_data_from_address(*DMC3_ADDRESS.read().unwrap() + INVENTORY_PTR)
}

pub(crate) fn read_data_from_address<T>(address: usize) -> T
where
    T: Copy,
{
    unsafe { *(address as *const T) }
}

/// Generic method to get the base address for the specified module, returns 0 if it doesn't exist
pub fn get_base_address(module_name: &str) -> usize {
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

/// Get current mission
pub fn get_mission() -> u8 {
    read_data_from_address(*DMC3_ADDRESS.read().unwrap() + 0xC8F250usize)
}

/// Get current room
pub fn get_room() -> i32 {
    read_data_from_address(*DMC3_ADDRESS.read().unwrap() + 0xC8F258usize)
}

/// Base address for DDMK
pub extern "system" fn get_mary_base_address() -> usize {
    get_base_address("Mary.dll")
}
