use std::ffi::{c_float, c_int};
use crate::utilities::get_mary_base_address;
use imgui_sys::{cty, ImGuiCond, ImGuiInputTextCallback, ImGuiInputTextFlags, ImGuiWindowFlags, ImVec2};
use std::os::raw::c_char;
use std::sync::OnceLock;

pub type ImGuiBegin =
    extern "C" fn(name: *const cty::c_char, p_open: *mut bool, flags: ImGuiWindowFlags) -> bool;
pub type ImGuiButton = extern "C" fn(label: *const cty::c_char, size: &ImVec2) -> bool;
pub type ImGuiText = extern "C" fn(text: *const cty::c_char, text_end: *const cty::c_char);
pub type ImGuiTextInput = extern "C" fn(
    label: *const cty::c_char,
    buf: *mut cty::c_char,
    buf_size: usize,
    flags: ImGuiInputTextFlags,
    callback: ImGuiInputTextCallback,
    user_data: *mut cty::c_void,
) -> bool;
//pub type ImGuiWindowPos = extern "C" fn(pos: &ImVec2, cond: ImGuiCond);
pub type ImGuiNextWindowPos = extern "C" fn (pos: &ImVec2, cond: ImGuiCond, pivot: &ImVec2);

pub type ImGuiSameLine = extern "C" fn (offset_from_start_x: c_float, spacing_w: c_float);
pub type ImGuiPushID = extern "C" fn (offset_from_start_x: c_int);

//pub type ImGuiCheckbox = extern "C" fn (text: *const cty::c_char, p_open: *mut bool);

pub const BEGIN_FUNC_ADDR: usize = 0x1F8B0;
pub const END_FUNC_ADDR: usize = 0x257B0;
pub const BUTTON_ADDR: usize = 0x59f20;
// 5cd0
pub const TEXT_ADDR: usize = 0x69d50;
pub const INPUT_ADDR: usize = 0x60c80;

pub const POS_FUNC_ADDR: usize = 0x374a0;
pub const SAME_LINE_ADDR: usize = 0x36200;
pub const PUSH_ID_ADDR: usize = 0x32850;
pub const POP_ID_ADDR: usize = 0x5fe0;
//pub const CHECKBOX_FUNC_ADDR: usize = 0x5a7e0;

pub fn text<T: AsRef<str>>(text: T) {
    let s = text.as_ref();
    unsafe {
        let start = s.as_ptr();
        let end = start.add(s.len());
        std::mem::transmute::<_, ImGuiText>(get_mary_base_address() + TEXT_ADDR)(start as *const c_char, end as *const c_char);
    }
}

pub fn input_rs<T: AsRef<str>>(label: T, buf: &mut String, password: bool) {
    crate::inputs::InputText::new(label, buf).password(password).build();
}

pub type BasicNothingFunc = unsafe extern "system" fn(); // No args no returns

static IMGUI_END: OnceLock<BasicNothingFunc> = OnceLock::new();
static IMGUI_BEGIN: OnceLock<ImGuiBegin> = OnceLock::new();
static IMGUI_BUTTON: OnceLock<ImGuiButton> = OnceLock::new();
static IMGUI_POS: OnceLock<ImGuiNextWindowPos> = OnceLock::new();
static IMGUI_SAME_LINE: OnceLock<ImGuiSameLine> = OnceLock::new();
static IMGUI_PUSH_ID: OnceLock<ImGuiPushID> = OnceLock::new();
static IMGUI_POP_ID: OnceLock<BasicNothingFunc> = OnceLock::new(); 
//static IMGUI_CHECKBOX: OnceLock<ImGuiNextWindowPos> = OnceLock::new();

// Helpers to retrieve values
pub fn get_imgui_end() -> &'static BasicNothingFunc {
    IMGUI_END.get_or_init(|| unsafe {
        std::mem::transmute::<_, BasicNothingFunc>(get_mary_base_address() + END_FUNC_ADDR)
    })
}

pub fn get_imgui_begin() -> &'static ImGuiBegin {
    IMGUI_BEGIN.get_or_init(|| unsafe {
        std::mem::transmute::<_, ImGuiBegin>(get_mary_base_address() + BEGIN_FUNC_ADDR)
    })
}

pub fn get_imgui_button() -> &'static ImGuiButton {
    IMGUI_BUTTON.get_or_init(|| unsafe {
        std::mem::transmute::<_, ImGuiButton>(get_mary_base_address() + BUTTON_ADDR)
    })
}

pub fn get_imgui_pos() -> &'static ImGuiNextWindowPos {
    IMGUI_POS.get_or_init(|| unsafe {
        std::mem::transmute::<_, ImGuiNextWindowPos>(get_mary_base_address() + POS_FUNC_ADDR)
    })
}

pub fn get_imgui_same_line() -> &'static ImGuiSameLine {
    IMGUI_SAME_LINE.get_or_init(|| unsafe {
        std::mem::transmute::<_, ImGuiSameLine>(get_mary_base_address() + SAME_LINE_ADDR)
    })
}

pub fn get_imgui_push_id() -> &'static ImGuiPushID {
    IMGUI_PUSH_ID.get_or_init(|| unsafe {
        std::mem::transmute::<_, ImGuiPushID>(get_mary_base_address() + PUSH_ID_ADDR)
    })
}

pub fn get_imgui_pop_id() -> &'static BasicNothingFunc {
    IMGUI_POP_ID.get_or_init(|| unsafe {
        std::mem::transmute::<_, BasicNothingFunc>(get_mary_base_address() + POP_ID_ADDR)
    })
}

// pub fn get_imgui_checkbox() -> &'static ImGuiCheckbox {
//     IMGUI_POS.get_or_init(|| unsafe {
//         std::mem::transmute::<_, ImGuiCheckbox>(get_mary_base_address() + CHECKBOX_FUNC_ADDR)
//     })
// }