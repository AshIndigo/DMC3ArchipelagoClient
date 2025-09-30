use crate::archipelago::ArchipelagoConnection;
use crate::constants::Status;
use crate::{archipelago, config};
use std::collections::HashMap;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{OnceLock, RwLock};

pub(crate) static CONNECTION_STATUS: AtomicIsize = AtomicIsize::new(0); // Disconnected
pub static CHECKLIST: OnceLock<RwLock<HashMap<String, bool>>> = OnceLock::new();

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
pub async fn send_connect_message(url: String, name: String, password: String) {
    log::debug!("Connecting to Archipelago");
    match archipelago::TX_ARCH.get() {
        None => log::error!("Connect TX doesn't exist"),
        Some(tx) => {
            tx.send(ArchipelagoConnection {
                url,
                name,
                password,
            })
            .await
            .expect("Failed to send data");
        }
    }
}

// #[tokio::main(flavor = "multi_thread", worker_threads = 1)]
// pub(crate) async fn disconnect_button_pressed() {
//     match archipelago::TX_DISCONNECT.get() {
//         None => log::error!("Disconnect TX doesn't exist"),
//         Some(tx) => {
//             tx.send(true).await.expect("Failed to send data");
//         }
//     }
// }

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

pub(crate) fn auto_connect() {
    loop {
        if CONNECTION_STATUS.load(Ordering::SeqCst) != 1 {
            log::debug!("Attempting to connect to local client");
            send_connect_message(
                format!(
                    "{}:{}",
                    (*config::CONFIG).connections.address,
                    (*config::CONFIG).connections.port
                ),
                "".parse().unwrap(),
                "".parse().unwrap(),
            );
        }
        std::thread::sleep(std::time::Duration::from_secs(
            (*config::CONFIG).connections.reconnect_interval_seconds as u64,
        ));
    }
}
