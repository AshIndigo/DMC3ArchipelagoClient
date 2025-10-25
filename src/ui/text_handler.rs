use crate::{create_hook, utilities};
use crate::utilities::{replace_single_byte, DMC3_ADDRESS};
use minhook::{MinHook, MH_STATUS};
use std::ptr::write_unaligned;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::{ptr};

pub static CANCEL_TEXT: AtomicBool = AtomicBool::new(false);
pub static LAST_OBTAINED_ID: AtomicU8 = AtomicU8::new(0);
static TEXT_DISPLAYED: LazyLock<usize> =
    LazyLock::new(|| *DMC3_ADDRESS + 0xCB89A0); // 0x01 if text is being displayed

pub unsafe fn setup_text_hooks() -> Result<(), MH_STATUS> {
    log::debug!("Setting up text related hooks");
    unsafe {
        create_hook!(
            DISPLAY_ITEM_GET_ADDR,
            replace_displayed_item_id,
            DISPLAY_ITEM_GET_SCREEN,
            "Modify Item Get Screen ID"
        );
        create_hook!(
            DISPLAY_ITEM_GET_DESTRUCTOR_ADDR,
            destroy_item_get_screen,
            DISPLAY_ITEM_GET_SCREEN_DESTRUCTOR,
            "Destroy Item Get Screen"
        );
        create_hook!(
            SETUP_ITEM_GET_SCREEN_ADDR,
            setup_item_get_screen,
            SETUP_ITEM_GET_SCREEN,
            "Setup Item Get Screen"
        );
    }
    Ok(())
}

pub unsafe fn disable_text_hooks(base_address: usize) -> Result<(), MH_STATUS> {
    log::debug!("Disabling text related hooks");
    const ADDRESSES: [usize; 3] = [DISPLAY_ITEM_GET_ADDR, DISPLAY_ITEM_GET_DESTRUCTOR_ADDR, SETUP_ITEM_GET_SCREEN_ADDR];
    unsafe {
        for addr in ADDRESSES {
            MinHook::disable_hook((base_address + addr) as *mut _)?;
        }
    }
    Ok(())
}

pub static DISPLAY_MESSAGE_VIA_INDEX: OnceLock<
    unsafe extern "C" fn(text_enabled: usize, message_index: i32),
> = OnceLock::new();

pub static GET_MESSAGE_START: OnceLock<
    unsafe extern "C" fn(ptr: usize, message_index: i32) -> usize,
> = OnceLock::new();

const UNUSED_INDEX: i32 = 11060; // Unused index, we like it
pub fn display_message_via_index(message: String) {
    unsafe {
        replace_unused_with_text(message);
        DISPLAY_MESSAGE_VIA_INDEX.get_or_init(|| {
            std::mem::transmute(*DMC3_ADDRESS + 0x2f08b0) // Offset to function
        })(*TEXT_DISPLAYED, UNUSED_INDEX);
    }
}

pub fn replace_unused_with_text(message: String) {
    let base = *DMC3_ADDRESS;
    unsafe {
        let message_begin: usize = GET_MESSAGE_START.get_or_init(|| {
            std::mem::transmute(base + 0x2F1180) // Offset to function
        })(base + 0xCB9340, UNUSED_INDEX);
        let message = message.replace("\n", "<BR>");
        let msg = format!("<PS 85 305><SZ 24><IT 0>{}<NE>\x00", message);
        let bytes = msg.as_bytes();
        if message_begin != 0 {
            ptr::copy_nonoverlapping(bytes.as_ptr(), message_begin as *mut u8, bytes.len());
        }
    }
}

pub static DISPLAY_ITEM_GET_SCREEN: OnceLock<unsafe extern "C" fn(ptr: usize)> = OnceLock::new();
pub(crate) const DISPLAY_ITEM_GET_ADDR: usize = 0x2955a0;
pub fn replace_displayed_item_id(item_get: usize) {
    if CANCEL_TEXT.load(Ordering::SeqCst) {
        let offset =  (*DMC3_ADDRESS + 0x2957e3) as *mut [u8; 6];
        utilities::modify_protected_memory(|| {
            unsafe {
                write_unaligned(
                    offset,
                    [0xBA, 60u8, 0x00, 0x00, 0x00, 0x90],
                );
                if let Some(original) = DISPLAY_ITEM_GET_SCREEN.get() {
                    original(item_get);
                }
                write_unaligned(
                    offset,
                    [0x8B, 0x93, 0x44, 0x09, 0x00, 0x00],
                );
            }
        }, offset).unwrap();
    } else {
        unsafe {
            if let Some(original) = DISPLAY_ITEM_GET_SCREEN.get() {
                original(item_get);
            }
        }
    }
}

pub const SETUP_ITEM_GET_SCREEN_ADDR: usize = 0x1B4750;
pub static SETUP_ITEM_GET_SCREEN: OnceLock<unsafe extern "C" fn(ptr: usize)> = OnceLock::new();

pub fn setup_item_get_screen(item_get: usize) {
    unsafe {
        if LAST_OBTAINED_ID.load(Ordering::SeqCst) != 0 {
            replace_single_byte(item_get + 0x36, LAST_OBTAINED_ID.load(Ordering::SeqCst));
        }
        if let Some(original) = SETUP_ITEM_GET_SCREEN.get() {
            original(item_get);
        }
    }
}

pub static DISPLAY_ITEM_GET_SCREEN_DESTRUCTOR: OnceLock<
    unsafe extern "C" fn(ptr: usize, param_1: u32),
> = OnceLock::new();
pub(crate) const DISPLAY_ITEM_GET_DESTRUCTOR_ADDR: usize = 0x295280;
pub fn destroy_item_get_screen(item_get: usize, _param_1: u32) {
    if CANCEL_TEXT.load(Ordering::SeqCst) {
        CANCEL_TEXT.store(false, Ordering::SeqCst);
    }
    unsafe {
        if let Some(original) = DISPLAY_ITEM_GET_SCREEN.get() {
            original(item_get);
        }
    }
}
