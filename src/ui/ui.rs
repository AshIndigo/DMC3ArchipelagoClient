use crate::archipelago::ArchipelagoConnection;
use crate::constants::Status;
use crate::{archipelago, bank, config};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::BufReader;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use std::{fs, path};

pub struct LoginData {
    pub(crate) archipelago_url: String,
    pub(crate) username: String,
    pub(crate) password: String,
}

impl LoginData {
    pub(crate) fn new() -> Self {
        Self {
            archipelago_url: String::with_capacity(256),
            username: String::with_capacity(256),
            password: String::with_capacity(256),
        }
    }
}

pub(crate) static CONNECTION_STATUS: AtomicIsize = AtomicIsize::new(0); // Disconnected
pub static LOGIN_DATA: OnceLock<Mutex<LoginData>> = OnceLock::new();
pub static CHECKLIST: OnceLock<RwLock<HashMap<String, bool>>> = OnceLock::new();
pub fn get_login_data() -> &'static Mutex<LoginData> {
    LOGIN_DATA.get_or_init(|| Mutex::new(LoginData::new()))
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
pub async fn connect_button_pressed(url: String, name: String, password: String) {
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

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
pub(crate) async fn disconnect_button_pressed() {
    match archipelago::TX_DISCONNECT.get() {
        None => log::error!("Disconnect TX doesn't exist"),
        Some(tx) => {
            tx.send(true).await.expect("Failed to send data");
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

pub fn load_login_data() -> Result<(), Box<dyn std::error::Error>> {
    if path::Path::new(archipelago::LOGIN_DATA_FILE).exists() {
        let login_data_file = fs::File::open(archipelago::LOGIN_DATA_FILE)?;
        let reader = BufReader::new(login_data_file);
        let mut json_reader = serde_json::Deserializer::from_reader(reader);
        let data = ArchipelagoConnection::deserialize(&mut json_reader)?;
        match get_login_data().lock() {
            Ok(mut instance) => {
                instance.archipelago_url = data.url;
                instance.username = data.name;
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    } else {
        Err("Failed to find login data file.\n\
        If this is a fresh install this is to be expected, the file will be created upon connection to a room.".into())
    }
}

pub(crate) fn auto_connect() {
    loop {
        if CONNECTION_STATUS.load(Ordering::SeqCst) != 1 {
            log::debug!("Attempting to connect to local client");
            connect_button_pressed(
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
