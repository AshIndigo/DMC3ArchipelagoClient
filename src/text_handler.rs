use crate::utilities::DMC3_ADDRESS;
use std::sync::atomic::AtomicBool;
use std::sync::{LazyLock, OnceLock, RwLock};
use std::time::{Duration, Instant};
use std::{ptr, thread};

pub static CANCEL_TEXT: AtomicBool = AtomicBool::new(false);
// const TEXT_PTR: usize = 0xCB89B8;
// const PTR_WRITE: usize = 0x00A38C03;
// const TEXT_LENGTH_ADDRESS: usize = 0xCB89E0; // X + 30 apparently?
// const TEXT_ADDRESS: usize = 0xCB8A0C; //0xCB8A1E; // Text string
//
// pub const RENDER_TEXT_ADDR: usize = 0x2f0440;
// pub static ORIGINAL_RENDER_TEXT: OnceLock<
//     unsafe extern "C" fn(
//         param_1: c_longlong,
//         param_2: c_longlong,
//         param_3: c_longlong,
//         param_4: c_longlong,
//     ),
// > = OnceLock::new();
//
// pub unsafe fn parry_text(
//     param_1: c_longlong,
//     param_2: c_longlong,
//     param_3: c_longlong,
//     param_4: c_longlong,
// ) {
//     //Parry text: param_1: 7ff7648d89a0, param_2: 120, param_3: 4140, param_4: 10000,
//     // Parry text: param_1: 7ff7648d89a0, param_2: 120, param_3: 140, param_4: 10000,
//     if param_1 == (*DMC3_ADDRESS.read().unwrap() + TEXT_DISPLAYED_ADDRESS) as c_longlong {
//         // This might only be compatible with ENG?
//         log::debug!(
//             "Parry text: param_1: {:x}, param_2: {:x}, param_3: {:x}, param_4: {:x},",
//             param_1,
//             param_2,
//             param_3,
//             param_4
//         );
//         if CANCEL_TEXT.load(Ordering::Relaxed) {
//             CANCEL_TEXT.store(false, Ordering::Relaxed);
//             return;
//         }
//     }
//     unsafe {
//         if let Some(original) = ORIGINAL_RENDER_TEXT.get() {
//             original(param_1, param_2, param_3, param_4);
//         }
//     }
// }

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

static TEXT_DISPLAYED: LazyLock<RwLock<usize>> =
    LazyLock::new(|| RwLock::new(*DMC3_ADDRESS.read().unwrap() + 0xCB89A0)); // 0x01 if text is being displayed

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
    let frame_duration = Duration::from_secs_f64(1.0 / TARGET_FPS);
    let text_displayed = *TEXT_DISPLAYED.read().unwrap();
    let timer_start = Instant::now();
    let message_ptr = message.as_ptr();
    // I don't really know what this does, but I think it helps with centering?
    let x_axis = unsafe {
        std::mem::transmute::<
            usize,
            extern "C" fn(param_1: usize, param_2: *const u8, param_3: u8) -> i32,
        >(*DMC3_ADDRESS.read().unwrap() + 0x2f1020)(text_displayed, message_ptr, 0)
    };

    let y_axis = -((x_axis & 0xffff) >> 1);
    while timer_start.elapsed() < duration {
        let frame_start = Instant::now();
        display_message(text_displayed, message_ptr, x_axis, y_axis, x_axis_mod, y_axis_mod);
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
            std::mem::transmute(*DMC3_ADDRESS.read().unwrap() + RENDER_TEXT_ADDR_NEW)
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

pub fn display_message_via_index(message: String) {
    const MESSAGE_BEGIN: usize = 0x1AC3113; // 0x1AC312C
    let message = message.replace("\n", "<BR>");
    let msg = format!("<PS 85 305><SZ 24><IT 0>{}<NE>\x00", message);
    let bytes = msg.as_bytes();
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), MESSAGE_BEGIN as *mut u8, bytes.len());
    }
    unsafe {
        DISPLAY_MESSAGE_VIA_INDEX.get_or_init(|| {
            std::mem::transmute(*DMC3_ADDRESS.read().unwrap() + 0x2f08b0) // Offset to function
        })(*TEXT_DISPLAYED.read().unwrap(), 11060); // Unused index, we like it
    }
}