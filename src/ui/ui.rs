//use crate::config::Settings;

use std::sync::atomic::Ordering;
use crate::{archipelago, bank};
use crate::archipelago::ArchipelagoData;
use crate::constants::Status;
use crate::hook::CONNECTION_STATUS;

pub struct ArchipelagoHud {
    // pub(crate) open: bool,
    // pub(crate) settings: Settings,
    pub(crate) archipelago_url: String,
    pub(crate) username: String,
    pub(crate) password: String,
    //pub input_handler: InputHandler,
}

impl ArchipelagoHud {
    pub(crate) fn new() -> Self {
        Self {
            // open: false,
            // settings: Settings::default(),
            archipelago_url: String::with_capacity(256),
            username: String::with_capacity(256),
            password: String::with_capacity(256),
            //input_handler: InputHandler::new(),
        }
    }
}

/*pub struct InputHandler {
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
            ui.button("Connect")
        });
}*/
#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
pub(crate) async fn connect_button_pressed(url: String, name: String, password: String) {
    match archipelago::TX_ARCH.get() {
        None => log::error!("Connect TX doesn't exist"),
        Some(tx) => {
            tx.send(ArchipelagoData {
                url,
                name,
                password,
            })
            .await
            .expect("Failed to send data");
        }
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
pub async fn retrieve_button_pressed(item_name: &str) {
    match bank::TX_BANK.get() {
        None => log::error!("Connect TX doesn't exist"),
        Some(tx) => {
            tx.send(item_name.parse().unwrap())
                .await
                .expect("Failed to send data");
        }
    }
}

pub fn get_status_text() -> &'static str {
    match CONNECTION_STATUS.load(Ordering::Relaxed).into() {
        Status::Connected => "Connected",
        Status::Disconnected => "Disconnected",
        Status::InvalidSlot => "Invalid slot (Check name)",
        Status::InvalidGame => "Invalid game (Wrong url/port or name?)",
        Status::IncompatibleVersion => "Incompatible Version, post on GitHub or Discord",
        Status::InvalidPassword => "Invalid password",
        Status::InvalidItemHandling => "Invalid item handling, post on Github or Discord",
    }
}