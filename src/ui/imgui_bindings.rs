use crate::ui::ddmk_hook::MARY_ADDRESS;
use crate::ui::ddmk_hook::USE_2022_DDMK;
use imgui_sys::{
    cty, ImGuiCond, ImGuiWindowFlags, ImVec2,
};
use std::os::raw::c_char;
use std::sync::OnceLock;

pub type ImGuiBegin =
    extern "C" fn(name: *const cty::c_char, p_open: *mut bool, flags: ImGuiWindowFlags) -> bool;
pub type ImGuiButton = extern "C" fn(label: *const cty::c_char, size: &ImVec2) -> bool;
pub type ImGuiText = extern "C" fn(text: *const cty::c_char, text_end: *const cty::c_char);
pub type ImGuiNextWindowPos = extern "C" fn(pos: &ImVec2, cond: ImGuiCond, pivot: &ImVec2);

pub const BEGIN_FUNC_ADDR: usize = if USE_2022_DDMK { 0x1f640 } else { 0x1F8B0 }; // Good?
pub const END_FUNC_ADDR: usize = if USE_2022_DDMK { 0x24cd0 } else { 0x257B0 }; // Good?

pub const BUTTON_ADDR: usize = if USE_2022_DDMK { 0x55920 } else { 0x59F80 }; //0x59f20;
// 5cd0
pub const TEXT_ADDR: usize = if USE_2022_DDMK { 0x65210 } else { 0x69db0 }; //0x69d50;

pub const NEXT_POS_FUNC_ADDR: usize = if USE_2022_DDMK { 0x351b0 } else { 0x374a0 };
pub const NEXT_WINDOW_SIZE_ADDR: usize = if USE_2022_DDMK { 0x35240 } else { 0x37530 };
//pub const CHECKBOX_FUNC_ADDR: usize = 0x5a7e0;

pub fn text<T: AsRef<str>>(text: T) {
    let s = text.as_ref();
    unsafe {
        let start = s.as_ptr();
        let end = start.add(s.len());
        std::mem::transmute::<_, ImGuiText>(*MARY_ADDRESS + TEXT_ADDR)(
            start as *const c_char,
            end as *const c_char,
        );
    }
}

pub type BasicNothingFunc = unsafe extern "system" fn(); // No args no returns

static IMGUI_END: OnceLock<BasicNothingFunc> = OnceLock::new();
static IMGUI_BEGIN: OnceLock<ImGuiBegin> = OnceLock::new();
static IMGUI_BUTTON: OnceLock<ImGuiButton> = OnceLock::new();
static IMGUI_POS: OnceLock<ImGuiNextWindowPos> = OnceLock::new();

// Helpers to retrieve values
pub fn get_imgui_end() -> &'static BasicNothingFunc {
    IMGUI_END.get_or_init(|| unsafe {
        std::mem::transmute::<_, BasicNothingFunc>(*MARY_ADDRESS + END_FUNC_ADDR)
    })
}

pub fn get_imgui_begin() -> &'static ImGuiBegin {
    IMGUI_BEGIN.get_or_init(|| unsafe {
        std::mem::transmute::<_, ImGuiBegin>(*MARY_ADDRESS + BEGIN_FUNC_ADDR)
    })
}

pub fn get_imgui_button() -> &'static ImGuiButton {
    IMGUI_BUTTON.get_or_init(|| unsafe {
        std::mem::transmute::<_, ImGuiButton>(*MARY_ADDRESS + BUTTON_ADDR)
    })
}

pub fn get_imgui_next_pos() -> &'static ImGuiNextWindowPos {
    IMGUI_POS.get_or_init(|| unsafe {
        std::mem::transmute::<_, ImGuiNextWindowPos>(*MARY_ADDRESS + NEXT_POS_FUNC_ADDR)
    })
}

pub fn get_imgui_next_size() -> &'static ImGuiNextWindowPos {
    IMGUI_POS.get_or_init(|| unsafe {
        std::mem::transmute::<_, ImGuiNextWindowPos>(*MARY_ADDRESS + NEXT_WINDOW_SIZE_ADDR)
    })
}