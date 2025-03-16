use crate::ddmk_hook::get_mary_base_address;
use imgui_sys::{cty, ImGuiInputTextCallback, ImGuiInputTextFlags, ImGuiWindowFlags, ImVec2};
use std::os::raw::c_char;

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

pub const BEGIN_FUNC_ADDR: usize = 0x1F8B0;
pub const END_FUNC_ADDR: usize = 0x257B0;
pub const BUTTON_ADDR: usize = 0x59f20;
// 5cd0
pub const TEXT_ADDR: usize = 0x69d50;
pub const INPUT_ADDR: usize = 0x60c80;

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