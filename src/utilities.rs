use std::ffi::{CString, OsStr};
use winapi::shared::minwindef::HINSTANCE;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::{PAGE_EXECUTE_READWRITE};
use std::{slice, thread};
use std::os::windows::ffi::OsStrExt;
use std::time::Duration;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::FindWindowA;

const TEXT_DISPLAYED_ADDRESS: usize = 0xCB89A0; // 0x01 if text is being displayed
const TEXT_LENGTH_ADDRESS: usize = 0xCB89E0; // X + 30 apparently?
const TEXT_ADDRESS: usize = 0xCB8A1E; // Text string

pub unsafe fn display_message(string: &str) { unsafe {
    let bytes = string.as_bytes();
    let ptr = TEXT_LENGTH_ADDRESS as *mut u8;

    std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
    *ptr.add(bytes.len()) = 0x00; // Null terminator
    std::ptr::write(TEXT_LENGTH_ADDRESS as *mut u8, (bytes.len() + 1) as u8);
    std::ptr::write(TEXT_DISPLAYED_ADDRESS as *mut u8, 0x01); // TODO Verify
}}

/// Read an int from DMC3
pub fn read_int_from_address(address: usize) -> i32 {
    unsafe { *((address + get_dmc3_base_address()) as *const i32) }
}

pub fn read_usize_from_address(address: usize) -> usize {
    unsafe { *((address + get_dmc3_base_address()) as *const usize) }
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

/// Get the base address for DMC3
pub fn get_dmc3_base_address() -> usize {
    get_base_address("dmc3.exe")
}

/// Checks to see if DDMK is loaded
// TODO Could be shortened?
pub fn is_ddmk_loaded() -> bool {
    let wide_name: Vec<u16> = OsStr::new("Mary.dll")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let module_handle: HINSTANCE = GetModuleHandleW(wide_name.as_ptr());
        !module_handle.is_null()
    }
}

pub unsafe fn replace_single_byte(offset: usize, new_val: u8) { unsafe {
    let length = 1;
    let offset = offset + get_dmc3_base_address();
    let mut old_protect = 0;
    VirtualProtect(
        offset as *mut _,
        length,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    );

    let table = slice::from_raw_parts_mut(offset as *mut u8, length);
    table[0] = new_val;

    VirtualProtect(offset as *mut _, length, old_protect, &mut old_protect);
    // log::debug!(
    //     "Modified byte at: Offset: {:x}, byte: {:x}",
    //     offset,
    //     new_val
    // );
}}

pub unsafe fn replace_single_byte_no_offset(offset: usize, new_val: u8) { unsafe {
    let length = 1;
    let offset = offset;
    let mut old_protect = 0;
    VirtualProtect(
        offset as *mut _,
        length,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    );

    let table = slice::from_raw_parts_mut(offset as *mut u8, length);
    table[0] = new_val;

    VirtualProtect(offset as *mut _, length, old_protect, &mut old_protect);
    // log::debug!(
    //     "Modified byte at: Offset: {:x}, byte: {:x}",
    //     offset,
    //     new_val
    // );
}}

/// Get current mission
pub fn get_mission() -> i32 {
    read_int_from_address(0xC8F250usize)
}

/// Get current room
pub(crate) fn get_room() -> i32 {
    read_int_from_address(0xC8F258usize)
}

/// Get a boolean from DDMK
pub fn read_bool_from_address_ddmk(address: usize) -> bool {
    unsafe { *((address + get_mary_base_address()) as *const bool) }
}

/// Base address for DDMK
// TODO Add check to make sure DDMK is loaded first?
pub extern "system" fn get_mary_base_address() -> usize {
    get_base_address("Mary.dll")
}

/// TODO May not be needed
/// Finds the HWND for DMC3 though
pub fn find_window_after_delay() -> Option<HWND> {
    let window_name = CString::new("Devil May Cry HD Collection").expect("CString creation failed");
    let window_name_pcstr = windows::core::PCSTR(window_name.as_ptr() as _);

    loop {
        unsafe {
            let hwnd = FindWindowA(None, window_name_pcstr);

            if let Ok(hwnd) = hwnd {
                // Window found
                return Some(hwnd);
            }
        }

        // Wait for 1 second before retrying
        thread::sleep(Duration::from_secs(1));
    }
}