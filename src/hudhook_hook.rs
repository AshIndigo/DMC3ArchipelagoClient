// use crate::config::Settings;
// use hudhook::hooks::dx11::ImguiDx11Hooks;
// use hudhook::renderer::keys::vk_to_imgui;
// use hudhook::windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, WPARAM};
// use hudhook::windows::Win32::UI::Input::KeyboardAndMouse::{
//     GetKeyboardState, MapVirtualKeyA, MAPVK_VK_TO_CHAR, VIRTUAL_KEY,
// };
// use hudhook::windows::Win32::UI::WindowsAndMessaging::CallWindowProcW;
// use hudhook::{eject, Hooks, Hudhook, ImguiRenderLoop, MessageFilter, RenderContext};
// use imgui::sys::ImGuiConfigFlags;
// use imgui::{Condition, Ui};
// use std::time::{Duration, Instant};
// use winapi::um::winuser::{GetAsyncKeyState, ToUnicode};
// use windows::Win32::UI::WindowsAndMessaging::ShowCursor;
// use crate::ui::{render_imgui_ui, show_cursor};
//
// pub fn start_imgui_hudhook(hinst_dll: HINSTANCE) {
//     if let Err(e) = Hudhook::builder()
//         .with::<ImguiDx11Hooks>(ArchipelagoHud::new())
//         .with_hmodule(hinst_dll)
//         .build()
//         .apply()
//     {
//         log::error!("Couldn't apply hooks: {e:?}");
//         eject();
//     }
//     log::info!("Started imgui_hudhook");
// }
//

//
// fn vk_to_char(vk: u32) -> Option<char> {
//     unsafe {
//         // This doesnt actually work because i cant use keyboardstate...
//         let mut keyboard_state: [u8; 256] = [0; 256];
//
//         match GetKeyboardState(&mut keyboard_state) {
//             _ => {}
//         }
//
//         // Buffer for translated characters
//         let mut char_buffer: [winapi::um::winnt::WCHAR; 2] = [0; 2];
//
//         let result = ToUnicode(
//             vk as u32,
//             0, // Scan code (0 for simplicity’s sake)
//             keyboard_state.as_ptr(),
//             char_buffer.as_mut_ptr(),
//             char_buffer.len() as i32,
//             0,
//         );
//
//         if result > 0 {
//             Some(char_buffer[0] as u8 as char)
//         } else {
//             None
//         }
//     }
// }
//
//
// impl ImguiRenderLoop for ArchipelagoHud {
//     fn before_render<'a>(
//         &'a mut self,
//         ctx: &mut imgui::Context,
//         _render_context: &'a mut dyn RenderContext,
//     ) {
//         // Iterate through all possible virtual key codes
//         for vk in 0..256 {
//             let is_pressed = unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0;
//             if vk < 0xFF && vk > 0x06 {
//                 // The lower ones capture mouse movement which isnt good for imgui
//                 if let Some(key) = vk_to_imgui(VIRTUAL_KEY(vk as _)) {
//                     //log::debug!("Got key: {:?}", key);
//                     if is_pressed {
//                         if self.input_handler.can_process_key() {
//                             ctx.io_mut().add_key_event(key, true);
//                             if self.open {
//                                 if let Some(character) = vk_to_char(vk as u32) {
//                                     ctx.io_mut().add_input_character(character);
//                                 }
//                             }
//                         }
//                     }
//                     if !is_pressed {
//                         ctx.io_mut().add_key_event(key, false);
//                     }
//                 }
//             }
//         }
//     }
//
//     fn render(&mut self, ui: &mut Ui) {
//         if ui.is_key_pressed(self.settings.display) {
//             self.open = !self.open;
//             unsafe {
//                 show_cursor(self.open);
//             }
//         }
//         if self.open {
//             render_imgui_ui(self, ui);
//         }
//     }
// }