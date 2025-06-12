use crate::archipelago::ArchipelagoData;
use crate::constants::Status;
use crate::hook::CONNECTION_STATUS;
use crate::{archipelago, bank};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock, RwLock};

pub struct ArchipelagoHud {
    pub(crate) archipelago_url: String,
    pub(crate) username: String,
    pub(crate) password: String,
}

impl ArchipelagoHud {
    pub(crate) fn new() -> Self {
        Self {
            archipelago_url: String::with_capacity(256),
            username: String::with_capacity(256),
            password: String::with_capacity(256),
        }
    }
}

pub static HUD_INSTANCE: OnceLock<Mutex<ArchipelagoHud>> = OnceLock::new();
pub static CHECKLIST: OnceLock<RwLock<HashMap<String, bool>>> = OnceLock::new();

pub fn get_hud_data() -> &'static Mutex<ArchipelagoHud> {
    HUD_INSTANCE.get_or_init(|| Mutex::new(ArchipelagoHud::new()))
}

pub fn set_checklist_item(item: &str, value: bool) {
    if let Some(rwlock) = CHECKLIST.get() {
        {
            let mut checklist = rwlock.write().unwrap();
            checklist.insert(item.to_string(), value);
        }
        if let Ok(_checklist) = rwlock.read() {}
    }
}

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
    match bank::TX_BANK_TO_INV.get() {
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
