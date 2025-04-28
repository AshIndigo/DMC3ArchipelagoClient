use crate::cache::{read_cache, CustomGameData};
use crate::check_handler::Location;
use crate::constants::CONSUMABLES;
use crate::hook::{modify_itm_table, Status};
use crate::{bank, cache, constants, generated_locations, hook};
use anyhow::anyhow;
use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use archipelago_rs::protocol::{DataStorageOperation, ServerMessage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs::{remove_file, File};
use std::io::{BufReader, Write};
use std::ops::SubAssign;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex, OnceLock, RwLock};

pub static MAPPING: OnceLock<Mapping> = OnceLock::new();
pub static DATA_PACKAGE: OnceLock<CustomGameData> = OnceLock::new();
pub static CHECKED_LOCATIONS: OnceLock<Mutex<Vec<String>>> = OnceLock::new(); // mut

pub static BANK: OnceLock<Mutex<HashMap<&'static str, i32>>> = OnceLock::new();

pub static TX_ARCH: OnceLock<Sender<ArchipelagoData>> = OnceLock::new();

pub static CONNECT_CHANNEL_SETUP: AtomicBool = AtomicBool::new(false);
pub static SLOT_NUMBER: AtomicI32 = AtomicI32::new(-1);
pub static TEAM_NUMBER: AtomicI32 = AtomicI32::new(-1);

pub fn get_checked_locations() -> &'static Mutex<Vec<String>> {
    CHECKED_LOCATIONS.get_or_init(|| Mutex::new(vec![]))
}

#[derive(Serialize, Deserialize)]
pub struct ArchipelagoData {
    pub url: String,
    pub name: String,
    #[serde(skip)]
    pub password: String,
}

impl Display for ArchipelagoData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(write!(
            f,
            "URL: {:#} Name: {:#} Password: {:#}",
            self.url, self.name, self.password
        )
        .expect("Failed to print connection data"))
    }
}

pub fn setup_connect_channel() -> Arc<Mutex<Receiver<ArchipelagoData>>> {
    let (tx, rx) = mpsc::channel();
    TX_ARCH.set(tx).expect("TX already initialized");
    Arc::new(Mutex::new(rx))
}

pub async fn connect_archipelago(
    login_data: ArchipelagoData,
) -> Result<ArchipelagoClient, ArchipelagoError> {
    #[allow(unused_assignments)]
    let mut client_res: Result<ArchipelagoClient, ArchipelagoError> = Err(ArchipelagoError::ConnectionClosed);
    if !cache::check_for_cache_file() {
        // If the cache file does not exist, then it needs to be acquired
        client_res = ArchipelagoClient::with_data_package(&login_data.url, Some(vec!["Devil May Cry 3".parse().expect("Failed to parse string")])).await;
        match client_res {
            Ok(ref cl) => match &cl.data_package() {
                // Write the data package to a local cache file
                None => {
                    log::error!("No data package found");
                    return Err(ArchipelagoError::ConnectionClosed);
                }
                Some(dp) => {
                    let mut clone_data = HashMap::new();
                    let _ = &dp.games.iter().for_each(|g| {
                        let dat = CustomGameData {
                            item_name_to_id: g.1.item_name_to_id.clone(),
                            location_name_to_id: g.1.location_name_to_id.clone(),
                        };
                        clone_data.insert(g.0.clone(), dat);
                    });
                    cache::write_cache(clone_data, cl.room_info())
                        .await
                        .unwrap_or_else(|err| log::error!("Failed to write cache: {}", err));
                }
            },
            Err(err) => return Err(err.into()),
        }
    } else {
        // If the cache exists, then connect normally and verify the cache file
        client_res = ArchipelagoClient::new(&login_data.url).await;
        match client_res {
            Ok(ref mut cl) => {
                match cache::find_checksum_errors(cl.room_info()).await {
                    None => log::info!("Checksums check out!"),
                    Some(failures) => {
                        // If there are checksums that don't match, obliterate the cache file and reconnect to obtain the data package
                        log::info!("Checksums check failures: {:?}", failures);
                        match remove_file("cache.json") {
                            Ok(_) => {}
                            Err(err) => {
                                log::error!("Failed to remove cache.json: {}", err);
                            }
                        };
                        //client_res = Err(ArchipelagoError::ConnectionClosed); // TODO Figure out a better way to do this - Good now?
                        return Box::pin(connect_archipelago(login_data)).await; //Err(anyhow!("Reconnecting to grab cache!"));
                    }
                }
            }
            Err(er) => return Err(er),
        }
    }
    log::info!("Connecting to url");
    match client_res {
        // Whether we have a client
        Ok(mut cl) => {
            log::info!("Attempting room connection");
            match cl.connect(
                "Devil May Cry 3",
                &login_data.name,
                Some(&login_data.password),
                Option::from(0b111),
                vec!["AP".to_string()],
                true,
            ).await {
                Ok(mut connected) => {
                    let Ok(mut checked_locations) = get_checked_locations().lock() else {
                        log::error!("Failed to get checked locations");
                        return Err(ArchipelagoError::ConnectionClosed);
                    };
                    checked_locations.clear(); // TODO Something weird happened here when reconnecting
                    let reversed_loc_id: HashMap<i32, String> = HashMap::from_iter(
                        read_cache()
                            .unwrap()
                            .location_name_to_id
                            .iter()
                            .map(|(k, v)| (*v, k.clone())),
                    );
                    connected.checked_locations.iter_mut().for_each(|val| {
                        checked_locations.push(reversed_loc_id.get(val).unwrap().clone());
                    });
                    log::info!("Connected info: {:?}", connected);
                    SLOT_NUMBER.store(connected.slot, Ordering::SeqCst);
                    TEAM_NUMBER.store(connected.team, Ordering::SeqCst);
                    save_connection_info(login_data).unwrap_or_else(|err| log::error!("Failed to save connection info: {}", err));
                    Ok(cl)
                }
                Err(err) => Err(err),
            }
        }

        Err(err) => Err(err),
    }
}

pub(crate) async fn sync_items(client: &mut ArchipelagoClient) {
    let id_to_name: HashMap<i32, String> = read_cache()
        .unwrap()
        .item_name_to_id
        .into_iter()
        .map(|(k, v)| (v, k))
        .collect();
    CHECKLIST.get().unwrap().write().unwrap().clear();
    match client.sync().await {
        Ok(received_items) => {
            for item in received_items.items {
                log::debug!("Network item: {:?}", item);
                set_checklist_item(id_to_name.get(&item.item).unwrap(), true);
            }
        }
        Err(err) => {
            log::error!("Failed to sync items: {}", err);
        }
    }
}

fn set_checklist_item(item: &str, value: bool) {
    if let Some(rwlock) = CHECKLIST.get() {
        {
            let mut checklist = rwlock.write().unwrap();
            checklist.insert(item.to_string(), value);
        }
        if let Ok(_checklist) = rwlock.read() {
            // log::debug!("Checklist: {:?}", *checklist);
        }
    }
}

fn save_connection_info(login_data: ArchipelagoData) -> Result<(), Box<dyn std::error::Error>> {
    let res = serde_json::to_string(&login_data)?;
    let mut file = File::create("login_data.json")?;
    file.write_all(res.as_bytes())?;
    Ok(())
}

pub async fn run_setup(cl: &mut ArchipelagoClient) {
    log::info!("Running setup");
    unsafe {
        hook::rewrite_mode_table();
    }
    match cl.data_package() {
        Some(_dat) => {
            log::info!("Using received data package");
            DATA_PACKAGE
                .set(CustomGameData {
                    item_name_to_id: cl
                        .data_package()
                        .unwrap()
                        .games
                        .get("Devil May Cry 3")
                        .unwrap()
                        .item_name_to_id
                        .clone(),
                    location_name_to_id: cl
                        .data_package()
                        .unwrap()
                        .games
                        .get("Devil May Cry 3")
                        .unwrap()
                        .location_name_to_id
                        .clone(),
                })
                .expect("DATA_PACKAGE already set");
        }
        None => {
            log::info!("No data package found, using cached data");
            DATA_PACKAGE
                .set(read_cache().expect("Expected cache file"))
                .expect("DATA_PACKAGE already set");
        }
    }
    // TODO Refactor the error handling + Use seed as some kind verification system? Ensure right mappings are being used?
    MAPPING
        .set(load_mappings_file().unwrap())
        .expect("MAPPING already set");
    use_mappings();
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Mapping {
    // For mapping JSON
    pub seed: String,
    pub slot: String,
    pub items: HashMap<String, String>,
    pub starter_items: Vec<String>,
}

pub struct ItemEntry {
    // Represents an item on the ground
    pub offset: usize,    // Offset for the item table
    pub room_number: u16, // Room number
    pub item_id: u8,      // Default Item ID
    pub mission: u8,      // Mission Number
    pub adjudicator: bool, // Adjudicator
                          // TODO Secret
}

fn use_mappings() {
    // TODO Need to see if the provided seed matches up with the world seed or something to ensure mappings are correct
    match MAPPING.get() {
        Some(data) => {
            for (location_name, item_name) in data.items.iter() {
                match generated_locations::ITEM_MISSION_MAP.get(location_name as &str) {
                    Some(entry) => match constants::get_item_id(item_name) {
                        Some(id) => unsafe { modify_itm_table(entry.offset, id) },
                        None => {
                            log::warn!("Item not found: {}", item_name);
                        }
                    },
                    None => {
                        log::warn!("Location not found: {}", location_name);
                    }
                }
            }
        }
        None => {
            log::error!("No mapping found");
        }
    }
}

pub fn load_mappings_file() -> Result<Mapping, Box<dyn std::error::Error>> {
    if Path::new("mappings.json").try_exists()? {
        log::info!("Mapping file Exists!");
        let mut json_reader =
            serde_json::Deserializer::from_reader(BufReader::new(File::open("mappings.json")?));
        Ok(Mapping::deserialize(&mut json_reader)?)
    } else {
        Err(Box::from(anyhow!("Mapping file doesn't exist")))
    }
}

pub async fn handle_things(
    client: &mut ArchipelagoClient,
    loc_rx: &Arc<Mutex<Receiver<Location>>>,
    bank_rx: &Arc<Mutex<Receiver<String>>>,
) {
    if let Ok(item_rec) = loc_rx.lock() {
        while let Ok(item) = item_rec.try_recv() {
            // See if there's an item!
            log::info!("Processing item: {}", item); // TODO Need to handle offline storage... if the item cant be sent it needs to be buffered
            let Some(mapping_data) = MAPPING.get() else {
                log::error!("No mapping found");
                return;
            };
            let Some(dp) = DATA_PACKAGE.get() else {
                log::error!("Data Package not found");
                return;
            };
            for (location_key, item_entry) in generated_locations::ITEM_MISSION_MAP.iter() {
                //log::debug!("Checking room {} vs {} and mission {} vs {}", v.room_number as i32, item.room, v.mission as i32, item.mission);
                if item_entry.room_number as i32 == item.room {
                    // && v.mission as i32 == item.mission { // First confirm the room and mission number
                    //log::debug!("Room and mission check out!");
                    let item_str = mapping_data.items.get(*location_key).unwrap();
                    log::debug!(
                        "Checking location items: 0x{:x} vs 0x{:x}",
                        constants::get_item_id(item_str).unwrap(),
                        item.item_id as u8
                    );
                    if constants::get_item_id(item_str).unwrap() == item.item_id as u8 {
                        // Then see if the item picked up matches the specified in the map
                        match dp.location_name_to_id.get(*location_key) {
                            Some(loc_id) => {
                                match client.location_checks(vec![loc_id.clone()]).await {
                                    Ok(_) => {
                                        if constants::KEY_ITEMS.contains(&&**item_str) {
                                            set_checklist_item(item_str, true);
                                            log::debug!("Key Item checked off: {}", item_str);
                                        }
                                        log::info!(
                                            "Location check successful: {} ({}), Item: {}",
                                            location_key,
                                            loc_id,
                                            item_str
                                        );
                                    }
                                    Err(err) => {
                                        log::info!("Failed to check location: {}", err);
                                    }
                                }
                            }
                            None => log::error!("Location not found: {}", location_key),
                        }
                    }
                }
            }
        }
    }
    //bank::handle_bank(bank_rx); // TODO Bank
    match client.recv().await {
        Ok(opt_msg) => match opt_msg {
            None => {}
            Some(ServerMessage::PrintJSON(json_msg)) => {
                log::info!("Printing JSON: {:?}", json_msg.data);
            }
            Some(ServerMessage::RoomInfo(_)) => {}
            Some(ServerMessage::ConnectionRefused(err)) => {
                // TODO Update UI status to mark as refused+reason
                log::error!("Connection refused: {:?}", err.errors);
            }
            Some(ServerMessage::Connected(_)) => {
                hook::CONNECTION_STATUS.store(Status::Connected.into(), Ordering::Relaxed);
            }
            Some(ServerMessage::ReceivedItems(items)) => {
                // READ https://github.com/ArchipelagoMW/Archipelago/blob/main/docs/network%20protocol.md#synchronizing-items
                for item in items.items.iter() {
                    /*    unsafe { // TODO This will crash if its on the main menu? or not prepared properly?
                        utilities::display_message(format!(
                            "Received {}!",
                            constants::get_item(item.item as u64)
                        ));
                    }*/
                    if item.item < 0x14 { // TODO Bank stuff broken
                        // Consumables/orbs TODO
                        //bank::add_item(client, item).await;
                    }
                }
                log::debug!("Received items: {:?}", items.items);
            }
            Some(ServerMessage::LocationInfo(_)) => {}
            Some(ServerMessage::RoomUpdate(_)) => {}
            Some(ServerMessage::Print(msg)) => {
                log::info!("Printing message: {:?}", msg);
            }
            Some(ServerMessage::DataPackage(_)) => {} // Ignore
            Some(ServerMessage::Bounced(_)) => {
                log::debug!("Boing!")
            }
            Some(ServerMessage::InvalidPacket(invalid_packet)) => {
                log::error!("Invalid packet: {:?}", invalid_packet);
            }
            Some(ServerMessage::Retrieved(_)) => {}
            Some(ServerMessage::SetReply(reply)) => {
                log::debug!("SetReply: {:?}", reply); // TODO Use this for the bank...
            }
        },
        Err(ArchipelagoError::NetworkError(err)) => {
            log::info!("Failed to receive data, reconnecting: {}", err);
            /* match connect_archipelago(ArchipelagoData {
                url: "".to_string(),
                name: "".to_string(),
                password: "".to_string(),
            }).await {
                Ok(client) => {
                    *cl = client;
                }
                Err(_) => {}
            }*/
        }
        Err(err) => {
            log::info!("Failed to receive data: {}", err)
        }
    }
}

// An ungodly mess, TODO Remove?
/*pub async fn connect_archipelago_get_url() -> Result<ArchipelagoClient, ArchipelagoError> {
    let url = input("Archipelago URL: ")?;
    let name = input("Name: ")?;
    let password = input("Password (Leave blank if unneeded): ")?;
    log::info!("url: {}", url);

    connect_archipelago(ArchipelagoData {
        url,
        name,
        password,
    })
    .await
}*/

/* // TODO Remove
fn input(text: &str) -> Result<String, anyhow::Error> {
    log::info!("{}", text);

    Ok(io::stdin().lock().lines().next().unwrap()?)
}*/
pub static CHECKLIST: OnceLock<RwLock<HashMap<String, bool>>> = OnceLock::new();
