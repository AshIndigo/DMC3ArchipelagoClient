use crate::create_hook;
use crate::utilities::{replace_single_byte, DMC3_ADDRESS};
use minhook::{MinHook, MH_STATUS};
use std::ptr::write_unaligned;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::time::{Duration, Instant};
use std::{ptr, thread};
use std::ops::Add;
use windows::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS};

pub static CANCEL_TEXT: AtomicBool = AtomicBool::new(false);
pub static LAST_OBTAINED_ID: AtomicU8 = AtomicU8::new(0);

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

pub static RENDER_TEXT: OnceLock<
    unsafe extern "C" fn(
        param_1: usize,
        param_2: *const u8,
        param_3: i32,
        param_4: i32,
        param_5: usize,
        param_6: usize,
        param_7: usize,
        param_8: u8,
    ),
> = OnceLock::new();

static TEXT_DISPLAYED: LazyLock<usize> =
    LazyLock::new(|| *DMC3_ADDRESS + 0xCB89A0); // 0x01 if text is being displayed

/// Arguments:
///
/// message: The message to display
///
/// duration: How long it should be displayed
///
/// x_axis_mod: How the message should be offset from the center of the screen via the X-axis
///
/// y_axis_mod: Similar to the above but for the Y-axis
pub fn display_text(message: &String, duration: Duration, x_axis_mod: i32, y_axis_mod: i32) {
    const TARGET_FPS: f64 = 60.0;
    let message = message.replace("\n", "<BR>");
    let message = message.add("<NE>\x00");
    let frame_duration = Duration::from_secs_f64(1.0 / TARGET_FPS);
    let text_displayed = *TEXT_DISPLAYED;
    let timer_start = Instant::now();
    let message_ptr = message.as_ptr();
    // I don't really know what this does, but I think it helps with centering?
    let x_axis = unsafe {
        std::mem::transmute::<
            usize,
            extern "C" fn(param_1: usize, param_2: *const u8, param_3: u8) -> i32,
        >(*DMC3_ADDRESS + 0x2f1020)(text_displayed, message_ptr, 0)
    };

    let y_axis = -((x_axis & 0xffff) >> 1);
    while timer_start.elapsed() < duration {
        let frame_start = Instant::now();
        display_message(
            text_displayed,
            message_ptr,
            x_axis,
            y_axis,
            x_axis_mod,
            y_axis_mod,
        );
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            // Need the delay or the game becomes very unhappy. But also too fast of a repeat deep-fries it
            thread::sleep(frame_duration - elapsed);
        }
    }
}

const RENDER_TEXT_ADDR_NEW: usize = 0x2f0b20;
fn display_message(
    text_displayed: usize,
    message_ptr: *const u8,
    x_axis: i32,
    y_axis: i32,
    x_axis_mod: i32,
    y_axis_mod: i32,
) {
    unsafe {
        RENDER_TEXT.get_or_init(|| {
            std::mem::transmute(*DMC3_ADDRESS + RENDER_TEXT_ADDR_NEW)
        })(
            text_displayed,
            message_ptr,                           // The string itself
            0x100 - (x_axis >> 0x11) + x_axis_mod, // X axis
            0x70 + y_axis + y_axis_mod,            // Y Axis
            0x80ffffff,                            // Unknown
            0,                                     // Text width? 0 Seems to be normal
            0xfffff5,                              // Unknown
            0,                                     // Unknown
        );
    }
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
        let base = *DMC3_ADDRESS;
        let offset = base + 0x2957e3;
        let mut old_protect = PAGE_PROTECTION_FLAGS::default();
        const LENGTH: usize = 6;
        unsafe {
            VirtualProtect(
                offset as *mut _,
                LENGTH,
                PAGE_EXECUTE_READWRITE,
                &mut old_protect,
            ).expect("Unable to replace displayed item id - Before");
            write_unaligned(
                offset as *mut [u8; LENGTH],
                [0xBA, 60u8, 0x00, 0x00, 0x00, 0x90],
            );
            if let Some(original) = DISPLAY_ITEM_GET_SCREEN.get() {
                original(item_get);
            }
            write_unaligned(
                offset as *mut [u8; LENGTH],
                [0x8B, 0x93, 0x44, 0x09, 0x00, 0x00],
            );
            VirtualProtect(offset as *mut _, LENGTH, old_protect, &mut old_protect).expect("Unable to replace displayed item id - After");
        }
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
