use std::time::{Duration, Instant};
use imgui::{Condition, Ui};
use windows::Win32::UI::WindowsAndMessaging::ShowCursor;
use crate::config::Settings;

pub struct ArchipelagoHud {
    pub(crate) open: bool,
    pub(crate) settings: Settings,
    pub(crate) arch_url: String,
    pub(crate) username: String,
    pub(crate) password: String,
    pub input_handler: InputHandler,
}

impl ArchipelagoHud {
    pub(crate) fn new() -> Self {
        Self {
            open: false,
            settings: Settings::default(),
            arch_url: String::with_capacity(256),
            username: String::with_capacity(256),
            password: String::with_capacity(256),
            input_handler: InputHandler::new(),
        }
    }
}

pub struct InputHandler {
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

pub unsafe fn show_cursor(state: bool) { unsafe {
    ShowCursor(state);
}}

// Putting this in a separate method just in case
pub fn render_imgui_ui(hud_instance: &mut ArchipelagoHud, ui: &mut Ui) {
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
