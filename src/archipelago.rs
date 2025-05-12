use crate::cache::{CustomGameData, read_cache};
use crate::check_handler::Location;
use crate::constants::{EventCode, GAME_NAME, Status};
use crate::hook::{CONNECTION_STATUS, modify_itm_table};
use crate::ui::ui::ArchipelagoHud;
use crate::utilities::get_mission;
use crate::{cache, constants, generated_locations, hook, utilities};
use anyhow::anyhow;
use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use archipelago_rs::protocol::{Connected, JSONMessagePart, PrintJSON, ServerMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs::{File, remove_file};
use std::io::{BufReader, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock, RwLock, mpsc};

pub static MAPPING: OnceLock<Mapping> = OnceLock::new();
pub static CHECKLIST: OnceLock<RwLock<HashMap<String, bool>>> = OnceLock::new();
pub static DATA_PACKAGE: OnceLock<CustomGameData> = OnceLock::new();
pub static CHECKED_LOCATIONS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
pub static HUD_INSTANCE: OnceLock<Mutex<ArchipelagoHud>> = OnceLock::new();
pub static CONNECTED: OnceLock<Mutex<Connected>> = OnceLock::new();

pub static BANK: OnceLock<Mutex<HashMap<&'static str, i32>>> = OnceLock::new();

pub static TX_ARCH: OnceLock<Sender<ArchipelagoData>> = OnceLock::new();

pub static CONNECT_CHANNEL_SETUP: AtomicBool = AtomicBool::new(false);
pub static SLOT_NUMBER: AtomicI32 = AtomicI32::new(-1);
pub static TEAM_NUMBER: AtomicI32 = AtomicI32::new(-1);

pub fn get_checked_locations() -> &'static Mutex<Vec<String>> {
    CHECKED_LOCATIONS.get_or_init(|| Mutex::new(vec![]))
}

pub fn get_hud_data() -> &'static Mutex<ArchipelagoHud> {
    HUD_INSTANCE.get_or_init(|| Mutex::new(ArchipelagoHud::new()))
}

pub fn get_connected() -> &'static Mutex<Connected> {
    CONNECTED.get_or_init(|| {
        Mutex::new(Connected {
            team: 0,
            slot: 0,
            players: vec![],
            missing_locations: vec![],
            checked_locations: vec![],
            slot_data: Default::default(),
            slot_info: Default::default(),
            hint_points: 0,
        })
    })
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

pub async fn get_archipelago_client(
    login_data: &ArchipelagoData,
) -> Result<ArchipelagoClient, ArchipelagoError> {
    if !cache::check_for_cache_file() {
        // If the cache file does not exist, then it needs to be acquired
        let cl = ArchipelagoClient::with_data_package(
            &login_data.url,
            Some(vec![GAME_NAME.parse().expect("Failed to parse string")]),
        )
        .await?;
        
        match &cl.data_package() {
            // Write the data package to a local cache file
            None => {
                log::error!("No data package found");
                Err(ArchipelagoError::ConnectionClosed)
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
                Ok(cl)
            }
        }
    } else {
        // If the cache exists, then connect normally and verify the cache file
        let cl = ArchipelagoClient::new(&login_data.url).await?;
        match cache::find_checksum_errors(cl.room_info()).await {
            None => {
                log::info!("Checksums check out!");
                Ok(cl)
            }
            Some(failures) => {
                // If there are checksums that don't match, obliterate the cache file and reconnect to obtain the data package
                log::info!("Checksums check failures: {:?}", failures);
                if let Err(err) = remove_file("cache.json") {
                    log::error!("Failed to remove cache.json: {}", err);
                };
                Box::pin(get_archipelago_client(login_data)).await
            }
        }
    }
}

pub async fn connect_archipelago(
    login_data: ArchipelagoData,
) -> Result<ArchipelagoClient, ArchipelagoError> {
    log::info!("Attempting room connection");
    let mut cl = get_archipelago_client(&login_data).await?;
    match cl
        .connect(
            GAME_NAME,
            &login_data.name,
            Some(&login_data.password),
            Option::from(0b111),
            vec!["AP".to_string()],
        )
        .await
    {
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
                checked_locations.push((&*reversed_loc_id.get(val).unwrap().clone()).parse().unwrap());
            });
            log::info!("Connected info: {:?}", connected);
            SLOT_NUMBER.store(connected.slot, Ordering::SeqCst);
            TEAM_NUMBER.store(connected.team, Ordering::SeqCst);
            save_connection_info(login_data)
                .unwrap_or_else(|err| log::error!("Failed to save connection info: {}", err));
            Ok(cl)
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
        if let Ok(_checklist) = rwlock.read() {}
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
                        .get(GAME_NAME)
                        .unwrap()
                        .item_name_to_id
                        .clone(),
                    location_name_to_id: cl
                        .data_package()
                        .unwrap()
                        .games
                        .get(GAME_NAME)
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
    pub items: HashMap<String, LocationData>,
    pub starter_items: Vec<String>,
    pub players: Vec<String>
}
#[derive(Deserialize, Serialize, Debug)]
pub struct LocationData {
    pub name: String,
    pub description: String,
}

pub struct ItemEntry {
    // Represents an item on the ground
    pub offset: usize,     // Offset for the item table
    pub room_number: u16,  // Room number
    pub item_id: u8,       // Default Item ID
    pub mission: u8,       // Mission Number
    pub adjudicator: bool, // Adjudicator
    pub x_coord: u32,
    pub y_coord: u32,
    pub z_coord: u32,
    // TODO Secret
}

fn use_mappings() {
    // TODO Need to see if the provided seed matches up with the world seed or something to ensure mappings are correct
    match MAPPING.get() {
        Some(data) => {
            for (location_name, location_data) in data.items.iter() {
                match generated_locations::ITEM_MISSION_MAP.get(location_name as &str) {
                    Some(entry) => match constants::get_item_id(&*location_data.name) {
                        Some(id) => unsafe {
                            if location_is_checked_and_end(location_name) {
                                modify_itm_table(entry.offset, hook::DUMMY_ID)
                            } else {
                                modify_itm_table(entry.offset, id)
                            }
                        },
                        None => {
                            log::warn!("Item not found: {}", location_data.name);
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

fn location_is_checked_and_end(location_key: &str) -> bool {
    match constants::EVENT_TABLES.get(&get_mission()) {
        None => false,
        Some(event_tables) => {
            for event_table in event_tables {
                if event_table.location == location_key {
                    for event in event_table.events.iter() {
                        if event.event_type == EventCode::END {
                            match get_checked_locations().lock() {
                                Ok(checked_locations) => {
                                    if checked_locations.contains(&location_key.to_string()) {
                                        return true;
                                    }
                                }
                                Err(err) => {
                                    log::error!("Failed to get checked locations: {}", err);
                                    return false;
                                }
                            }
                        }
                    }
                }
            }
            false
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
    match handle_item_receive(client, loc_rx).await {
        Ok(_) => {}
        Err(err) => {
            log::error!("Failed to check location: {}", err);
        }
    };
    //bank::handle_bank(bank_rx); // TODO Bank
    match client.recv().await {
        Ok(opt_msg) => match opt_msg {
            None => {}
            Some(ServerMessage::PrintJSON(json_msg)) => {
                log::info!("{}", handle_print_json(json_msg));
            }
            Some(ServerMessage::RoomInfo(_)) => {}
            Some(ServerMessage::ConnectionRefused(err)) => {
                // TODO Update UI status to mark as refused+reason
                log::error!("Connection refused: {:?}", err.errors);
            }
            Some(ServerMessage::Connected(_)) => {
                CONNECTION_STATUS.store(Status::Connected.into(), Ordering::Relaxed);
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
                log::info!("Printing message: {}", msg.text);
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
            match get_hud_data().lock() {
                Ok(data) => match connect_archipelago(ArchipelagoData {
                    url: data.arch_url.clone(),
                    name: data.username.clone(),
                    password: data.username.clone(),
                })
                .await
                {
                    Ok(cl) => *client = cl,
                    Err(err) => log::error!("Failed to connect: {}", err),
                },
                Err(err) => {
                    log::error!("Failed to get hud data: {}", err);
                }
            }
        }
        Err(ArchipelagoError::IllegalResponse { received, expected }) => {
            log::error!(
                "Illegal response, expected {:#?}, got {:?}",
                expected,
                received
            );
        }
        Err(ArchipelagoError::ConnectionClosed) => {
            CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::Relaxed);
            log::info!("Connection closed"); // TODO Update status?
        }
        Err(ArchipelagoError::FailedSerialize(err)) => {
            log::error!("Failed to serialize message: {}", err);
        }
        Err(ArchipelagoError::NonTextWebsocketResult(msg)) => {
            log::error!("Non-text websocket result: {:?}", msg.into_data());
        }
    }
}

async fn handle_item_receive(
    client: &mut ArchipelagoClient,
    loc_rx: &Arc<Mutex<Receiver<Location>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(item_rec) = loc_rx.lock() {
        while let Ok(received_item) = item_rec.try_recv() {
            // See if there's an item!
            log::info!("Processing item: {}", received_item); // TODO Need to handle offline storage... if the item cant be sent it needs to be buffered
            let Some(mapping_data) = MAPPING.get() else {
                return Err(Box::from(anyhow!("No mapping found")));
            };
            let Some(dp) = DATA_PACKAGE.get() else {
                return Err(Box::from(anyhow!("Data package not found!")));
            };
            for (location_key, item_entry) in generated_locations::ITEM_MISSION_MAP.iter() {
                //log::debug!("Checking room {} vs {} and mission {} vs {}", v.room_number as i32, item.room, v.mission as i32, item.mission);
                if item_entry.room_number as i32 == received_item.room {
                    if item_entry.x_coord == 0
                        || (item_entry.x_coord == received_item.x_coord
                            && item_entry.y_coord == received_item.y_coord
                            && item_entry.z_coord == received_item.z_coord)
                    {
                        let location_data = mapping_data.items.get(*location_key).unwrap();
                        log::debug!("Believe this to be: {}", location_key);
                        log::debug!(
                            "Checking location items: 0x{:x} vs 0x{:x}",
                            constants::get_item_id(&*location_data.name).unwrap(),
                            received_item.item_id as u8
                        );
                        if constants::get_item_id(&*location_data.name).unwrap() == received_item.item_id as u8
                        {
                            // Then see if the item picked up matches the specified in the map
                            match dp.location_name_to_id.get(*location_key) {
                                Some(loc_id) => {
                                    edit_end_event(*location_key);
                                    unsafe { utilities::display_message(&location_data.description); }
                                    client.location_checks(vec![loc_id.clone()]).await?;
                                    if constants::KEY_ITEMS.contains(&&*location_data.name) {
                                        set_checklist_item(&*location_data.name, true);
                                        log::debug!("Key Item checked off: {}", location_data.name);
                                    }
                                    log::info!(
                                        "Location check successful: {} ({}), Item: {}",
                                        location_key,
                                        loc_id,
                                        location_data.name
                                    );
                                }
                                None => {
                                    Err(anyhow::anyhow!("Location not found: {}", location_key))?
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn edit_end_event(location_key: &str) {
    match constants::EVENT_TABLES.get(&get_mission()) {
        None => {}
        Some(event_tables) => {
            for event_table in event_tables {
                if event_table.location == location_key {
                    for event in event_table.events.iter() {
                        if event.event_type == EventCode::END {
                            unsafe {
                                utilities::replace_single_byte_no_offset(
                                    constants::EVENT_TABLE_ADDR + event.offset,
                                    0x00,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

fn handle_print_json(print_json: PrintJSON) -> String {
    log::debug!("Printing json: {:?}", print_json);
    let mut final_message: String = "".to_string();
    match print_json {
        PrintJSON::ItemSend {
            data,
            receiving: _receiving,
            item: _item,
        } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::ItemCheat {
            data,
            receiving: _receiving,
            item: _item,
            team: _team,
        } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Hint {
            data,
            receiving: _receiving,
            item: _item,
            found: _found,
        } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Join {
            data,
            team: _team,
            slot: _slot,
            tags: _tags,
        } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Part { data, team: _team, slot: _slot } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Chat {
            data,
            team: _team,
            slot: _slot,
            message: _message,
        } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::ServerChat { data, message: _message } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Tutorial { data } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::TagsChanged {
            data,
            team: _team,
            slot: _slot,
            tags: _tags,
        } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::CommandResult { data } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::AdminCommandResult { data } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Goal { data, team: _team, slot: _slot } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Release { data, team: _team, slot: _slot } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Collect { data, team: _team, slot: _slot } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Countdown { data, countdown: _countdown } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
    }
    final_message
}

fn handle_message_part(message: JSONMessagePart) -> String {
    match message {
        JSONMessagePart::PlayerId { text, player } => {
            log::debug!("PlayerId: {} i32: {}", text, player);
            get_connected().lock().unwrap().players[player as usize]
                .name
                .clone() // TODO I think I need to parse text as string?
        }
        JSONMessagePart::PlayerName { text } => text,
        JSONMessagePart::ItemId {
            text,
            flags: _flags,
            player: _player,
        } => constants::get_item(text.parse::<u64>().expect("Unable to parse as u64"))
            .parse()
            .unwrap(),
        JSONMessagePart::ItemName {
            text,
            flags,
            player,
        } => {
            log::debug!("ItemName: {:?} Flags: {}, Player: {}", text, flags, player);
            text
        }
        JSONMessagePart::LocationId { text, player } => {
            //let map: HashMap<i32, String> = DATA_PACKAGE.get().unwrap().location_name_to_id.iter().map(|(k, v)| (v, k)).collect();
            log::debug!("LocationId: {:?} Player: {}", text, player);
            text
        }
        JSONMessagePart::LocationName { text, player } => {
            log::debug!("LocationName: {:?}, Player: {}", text, player);
            text
        }
        JSONMessagePart::EntranceName { text } => text,
        JSONMessagePart::Color { text, color } => {
            log::debug!("Received color: txt:{}, color: {:?}", text, color);
            "".parse().unwrap()
        }
        JSONMessagePart::Text { text } => text,
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

/*fn input(text: &str) -> Result<String, anyhow::Error> {
    log::info!("{}", text);

    Ok(io::stdin().lock().lines().next().unwrap()?) // Use this to support command sending
}*/
