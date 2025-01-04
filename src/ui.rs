use crate::config::Settings;
use hudhook::hooks::dx11::ImguiDx11Hooks;
use hudhook::renderer::keys::vk_to_imgui;
use hudhook::windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, WPARAM};
use hudhook::windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardState, MapVirtualKeyA, MAPVK_VK_TO_CHAR, VIRTUAL_KEY,
};
use hudhook::windows::Win32::UI::WindowsAndMessaging::CallWindowProcW;
use hudhook::{eject, Hooks, Hudhook, ImguiRenderLoop, MessageFilter, RenderContext};
use imgui::sys::ImGuiConfigFlags;
use imgui::{
    Condition, InputTextCallback, InputTextCallbackHandler, InputTextFlags, Io, Key,
    TextCallbackData, Ui,
};
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use winapi::shared::minwindef::BYTE;
use winapi::um::winuser;
use winapi::um::winuser::{GetAsyncKeyState, ToUnicode, VK_F1};
use windows::Win32::UI::WindowsAndMessaging::ShowCursor;

pub fn start_imgui_hudhook(hinst_dll: HINSTANCE) {
    if let Err(e) = Hudhook::builder()
        .with::<ImguiDx11Hooks>(ArchipelagoHud::new())
        .with_hmodule(hinst_dll)
        .build()
        .apply()
    {
        log::error!("Couldn't apply hooks: {e:?}");
        eject();
    }
    log::info!("Started imgui_hudhook");
}

pub struct ArchipelagoHud {
    start_time: Instant,
    open: bool,
    settings: Settings,
    arch_url: String,
    input_handler: InputHandler,
}

impl ArchipelagoHud {
    pub(crate) fn new() -> Self {
        Self {
            start_time: Instant::now(),
            open: false,
            settings: Settings::default(),
            arch_url: String::with_capacity(256),
            input_handler: InputHandler::new(),
        }
    }
}

fn vk_to_char(vk: u32) -> Option<char> {
    unsafe {
        // This doesnt actually work because i cant use keyboardstate...
        let mut keyboard_state: [u8; 256] = [0; 256];

        match GetKeyboardState(&mut keyboard_state) {
            _ => {}
        }

        // Buffer for translated characters
        let mut char_buffer: [winapi::um::winnt::WCHAR; 2] = [0; 2];

        let result = ToUnicode(
            vk as u32,
            0, // Scan code (0 for simplicity’s sake)
            keyboard_state.as_ptr(),
            char_buffer.as_mut_ptr(),
            char_buffer.len() as i32,
            0,
        );

        if result > 0 {
            Some(char_buffer[0] as u8 as char)
        } else {
            None
        }
    }
}

struct InputHandler {
    last_key_time: Instant,
    delay: Duration,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            last_key_time: Instant::now(),
            delay: Duration::from_millis(100),
        }
    }

    pub fn can_process_key(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_key_time) >= self.delay {
            self.last_key_time = now;
            return true;
        }

        false // Ignore the key if it's too soon
    }
}

impl ImguiRenderLoop for ArchipelagoHud {
    fn before_render<'a>(
        &'a mut self,
        ctx: &mut imgui::Context,
        _render_context: &'a mut dyn RenderContext,
    ) {
        // Iterate through all possible virtual key codes
        for vk in 0..256 {
            let is_pressed = unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0;
            if vk < 0xFF && vk > 0x06 {
                // The lower ones capture mouse movement which isnt good for imgui
                if let Some(key) = vk_to_imgui(VIRTUAL_KEY(vk as _)) {
                    //log::debug!("Got key: {:?}", key);
                    if is_pressed {
                        if self.input_handler.can_process_key() {
                            ctx.io_mut().add_key_event(key, true);
                            if self.open {
                                if let Some(character) = vk_to_char(vk as u32) {
                                    ctx.io_mut().add_input_character(character);
                                }
                            }
                        }
                    }
                    if !is_pressed {
                        ctx.io_mut().add_key_event(key, false);
                    }
                }
            }
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        if ui.is_key_pressed(self.settings.display) {
            self.open = !self.open;
            unsafe {
                show_cursor(self.open);
            }
        }
        if self.open {
            render_imgui_ui(self, ui);
        }
    }
}

unsafe fn show_cursor(state: bool) {
    ShowCursor(state);
}

// Putting this in a separate method just in case
pub fn render_imgui_ui(hud_instance: &mut ArchipelagoHud, ui: &mut imgui::Ui) {
    ui.window("##hello")
        .size([320., 200.], Condition::FirstUseEver)
        .build(|| {
            ui.input_text("Archipelago URL", &mut hud_instance.arch_url)
                .build();
            ui.button("Connect").then(connect)
        });
}

fn connect() {
    log::info!("Connect");
}
