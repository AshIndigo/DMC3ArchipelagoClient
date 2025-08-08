use crate::bank::{get_bank, get_bank_key};
use crate::cache::read_cache;
use crate::check_handler::Location;
use crate::constants::{EventCode, ItemCategory, Status, GAME_NAME, ITEM_ID_MAP};
use crate::data::generated_locations;
use crate::ui::ui;
use crate::ui::ui::CONNECTION_STATUS;
use crate::utilities::{get_mission, read_data_from_address, DMC3_ADDRESS};
use crate::{
    bank, cache, constants, hook, item_sync, mapping, save_handler, text_handler, utilities,
};
use anyhow::anyhow;
use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use archipelago_rs::protocol::{
    Bounced, Connected, DataPackageObject, JSONColor, JSONMessagePart, NetworkItem, PrintJSON,
    Retrieved, ServerMessage,
};
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{remove_file, File};
use std::io::Write;
use std::ptr::write_unaligned;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock, RwLock, RwLockReadGuard};
use std::time::Duration;
use tokio::sync;
use tokio::sync::mpsc::Receiver;

static DATA_PACKAGE: Lazy<RwLock<Option<DataPackageObject>>> = Lazy::new(|| RwLock::new(None));

pub static CHECKED_LOCATIONS: OnceLock<Mutex<Vec<&'static str>>> = OnceLock::new();
pub static CONNECTED: OnceLock<Mutex<Connected>> = OnceLock::new();
pub static TX_ARCH: OnceLock<sync::mpsc::Sender<ArchipelagoConnection>> = OnceLock::new();
pub static TX_DISCONNECT: OnceLock<sync::mpsc::Sender<bool>> = OnceLock::new();

pub static SLOT_NUMBER: AtomicI32 = AtomicI32::new(-1);
pub static TEAM_NUMBER: AtomicI32 = AtomicI32::new(-1);

pub fn get_checked_locations() -> &'static Mutex<Vec<&'static str>> {
    CHECKED_LOCATIONS.get_or_init(|| Mutex::new(vec![]))
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

pub(crate) const LOGIN_DATA_FILE: &str = "login_data.json";

fn save_connection_info(login_data: ArchipelagoConnection) -> Result<(), Box<dyn Error>> {
    let res = serde_json::to_string(&login_data)?;
    let mut file = File::create(LOGIN_DATA_FILE)?;
    file.write_all(res.as_bytes())?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct ArchipelagoConnection {
    pub url: String,
    pub name: String,
    #[serde(skip)]
    pub password: String,
}

impl Display for ArchipelagoConnection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(write!(f, "URL: {:#} Name: {:#}", self.url, self.name, )
            .expect("Failed to print connection data"))
    }
}

pub fn setup_connect_channel() -> Receiver<ArchipelagoConnection> {
    let (tx, rx) = sync::mpsc::channel(8);
    TX_ARCH.set(tx).expect("TX already initialized");
    rx
}

pub fn setup_disconnect_channel() -> Receiver<bool> {
    let (tx, rx) = sync::mpsc::channel(8);
    TX_DISCONNECT.set(tx).expect("TX already initialized");
    rx
}

pub async fn get_archipelago_client(
    login_data: &ArchipelagoConnection,
) -> Result<ArchipelagoClient, ArchipelagoError> {
    if !cache::check_for_cache_file() {
        // If the cache file does not exist, then it needs to be acquired
        let cl = ArchipelagoClient::with_data_package(
            &login_data.url,
            None, //Some(vec![GAME_NAME.parse().expect("Failed to parse string")]),
        )
            .await?;
        match &cl.data_package() {
            // Write the data package to a local cache file
            None => {
                log::error!("No data package found");
                Err(ArchipelagoError::ConnectionClosed)
            }
            Some(dp) => {
                log::debug!("Data package rec: {:?}", dp);
                cache::write_cache(&dp)
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
                if let Err(err) = remove_file(cache::CACHE_FILENAME) {
                    log::error!("Failed to remove {}: {}", cache::CACHE_FILENAME, err);
                };
                Box::pin(get_archipelago_client(login_data)).await
            }
        }
    }
}

pub async fn connect_archipelago(
    login_data: ArchipelagoConnection,
) -> Result<ArchipelagoClient, ArchipelagoError> {
    log::info!("Attempting room connection");
    let mut cl = get_archipelago_client(&login_data).await?;
    let connected = cl
        .connect(
            GAME_NAME,
            &login_data.name,
            Some(&login_data.password),
            Option::from(0b111),
            vec!["AP".to_string()],
        )
        .await?;
    *get_connected().lock().expect("Failed to get connected") = connected;
    let Ok(mut checked_locations) = get_checked_locations().lock() else {
        log::error!("Failed to get checked locations");
        return Err(ArchipelagoError::ConnectionClosed);
    };
    checked_locations.clear(); // TODO Something weird happened here when reconnecting
    let reversed_loc_id: HashMap<i64, String> = HashMap::from_iter(
        read_cache()
            .unwrap()
            .games
            .get(GAME_NAME)
            .unwrap()
            .location_name_to_id
            .iter()
            .map(|(k, v)| (*v, k.clone())),
    );
    log::debug!("Attempting to send offline checks");
    item_sync::send_offline_checks(&mut cl).await.unwrap();
    let mut connected = get_connected().lock().unwrap();
    connected.checked_locations.iter_mut().for_each(|val| {
        checked_locations.push(
            generated_locations::ITEM_MISSION_MAP
                .get_key_value(reversed_loc_id.get(val).unwrap().as_str())
                .unwrap()
                .0,
        );
    });
    log::info!("Connected info: {:?}", connected);
    SLOT_NUMBER.store(connected.slot, Ordering::SeqCst);
    TEAM_NUMBER.store(connected.team, Ordering::SeqCst);
    save_connection_info(login_data)
        .unwrap_or_else(|err| log::error!("Failed to save connection info: {}", err));
    Ok(cl)
}

/// This is run when a there is a valid connection to a room.
pub async fn run_setup(cl: &mut ArchipelagoClient) -> Result<(), Box<dyn Error>> {
    log::info!("Running setup");
    hook::rewrite_mode_table();
    match cl.data_package() {
        // Set the data package global based on received or cached values
        Some(data_package) => {
            log::info!("Using received data package");
            set_data_package(data_package.clone());
        }
        None => {
            log::info!("No data package data received, using cached data");
            set_data_package(read_cache().expect("Expected cache file"));
        }
    }

    let mut sync_data = item_sync::get_sync_data().lock()?;
    *sync_data = item_sync::read_save_data().unwrap_or_default();
    if sync_data
        .room_sync_info
        .contains_key(&item_sync::get_index(&cl))
    {
        item_sync::CURRENT_INDEX.store(
            sync_data
                .room_sync_info
                .get(&item_sync::get_index(&cl))
                .unwrap()
                .sync_index,
            Ordering::SeqCst,
        );
    } else {
        item_sync::CURRENT_INDEX.store(0, Ordering::SeqCst);
    }

    hook::install_initial_functions(); // Hooks needed to modify the game
    match mapping::parse_slot_data() {
        Ok(_) => {
            log::info!("Successfully parsed mapping information");
            log::debug!("Mapping data: {:#?}", mapping::MAPPING.read().unwrap());
        }
        Err(err) => {
            return Err(format!("Failed to load mappings from slot data, aborting: {}", err).into());
        }
    }
    mapping::use_mappings()?;
    save_handler::create_special_save()?;
    Ok(())
}

pub(crate) fn location_is_checked_and_end(location_key: &str) -> bool {
    match constants::EVENT_TABLES.get(&get_mission()) {
        None => false,
        Some(event_tables) => {
            for event_table in event_tables {
                if event_table.location == location_key {
                    for event in event_table.events.iter() {
                        if event.event_type == EventCode::END {
                            match get_checked_locations().lock() {
                                Ok(checked_locations) => {
                                    if checked_locations.contains(&location_key) {
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

pub async fn handle_things(
    client: &mut ArchipelagoClient,
    loc_rx: &mut Receiver<Location>,
    bank_rx: &mut Receiver<String>,
    connect_rx: &mut Receiver<ArchipelagoConnection>,
    add_bank_rx: &mut Receiver<NetworkItem>,
    disconnect_request: &mut Receiver<bool>,
) {
    loop {
        tokio::select! {
            Some(message) = loc_rx.recv() => {
                if let Err(err) = handle_item_receive(client, message).await {
                    log::error!("Failed to handle item receive: {}", err);
                }
            }
            Some(message) = bank_rx.recv() => {
                if let Err(err) = bank::handle_bank(client, message).await {
                    log::error!("Failed to handle bank: {}", err);
                }
            }
            Some(message) = add_bank_rx.recv() => {
                 if let Err(err) = bank::add_item_to_bank(client, &message).await {
                    log::error!("Failed to add item to bank: {}", err);
                }
            }
            Some(reconnect_request) = connect_rx.recv() => {
                log::warn!("Reconnect requested while connected: {}", reconnect_request);
                break; // Exit to trigger reconnect in spawn_arch_thread
            }
            Some(_disconnect_request) = disconnect_request.recv() => {
                disconnect(client).await;
                break;
            }
            message = client.recv() => {
                if let Err(err) = handle_client_messages(message, client).await {
                    log::error!("Client error or disconnect: {}", err);
                    break; // Exit to allow clean reconnect
                }
            }
        }
    }
}

async fn disconnect(client: &mut ArchipelagoClient) {
    log::info!("Disconnecting");
    match client.disconnect(None).await {
        Ok(_) => {
            CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::Relaxed);
            TEAM_NUMBER.store(-1, Ordering::SeqCst);
            SLOT_NUMBER.store(-1, Ordering::SeqCst);
            log::info!("Disconnected from Archipelago room");
        }
        Err(err) => {
            log::error!("Failed to disconnect: {}", err);
        }
    }
    match hook::disable_hooks() {
        Ok(_) => {
            log::debug!("Disabled hooks");
        }
        Err(e) => {
            log::error!("Failed to disable hooks: {:?}", e);
        }
    }
    hook::restore_item_table();
    hook::restore_mode_table();
    log::info!("Game restored to default state");
}

async fn handle_client_messages(
    result: Result<Option<ServerMessage>, ArchipelagoError>,
    client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn Error>> {
    match result {
        Ok(opt_msg) => match opt_msg {
            None => Ok(()),
            Some(ServerMessage::PrintJSON(json_msg)) => {
                log::info!("{}", handle_print_json(json_msg));
                Ok(())
            }
            Some(ServerMessage::RoomInfo(_)) => Ok(()),
            Some(ServerMessage::ConnectionRefused(err)) => {
                // TODO Update UI status to mark as refused+reason
                CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::Relaxed);
                log::error!("Connection refused: {:?}", err.errors);
                Ok(())
            }
            Some(ServerMessage::Connected(_)) => {
                CONNECTION_STATUS.store(Status::Connected.into(), Ordering::Relaxed);
                Ok(())
            }
            Some(ServerMessage::ReceivedItems(items)) => {
                item_sync::handle_received_items_packet(items, client).await
            }
            Some(ServerMessage::LocationInfo(_)) => Ok(()),
            Some(ServerMessage::RoomUpdate(_)) => Ok(()),
            Some(ServerMessage::Print(msg)) => {
                log::info!("Printing message: {}", msg.text);
                Ok(())
            }
            Some(ServerMessage::DataPackage(_)) => Ok(()), // Ignore
            Some(ServerMessage::Bounced(bounced_msg)) => handle_bounced(bounced_msg, client).await,
            Some(ServerMessage::InvalidPacket(invalid_packet)) => {
                log::error!("Invalid packet: {:?}", invalid_packet);
                Ok(())
            }
            Some(ServerMessage::Retrieved(retrieved)) => handle_retrieved(retrieved),
            Some(ServerMessage::SetReply(reply)) => {
                log::debug!("SetReply: {:?}", reply);
                let mut bank = get_bank().write().unwrap();
                for item in constants::get_items_by_category(ItemCategory::Consumable).iter() {
                    if item.eq(&reply.key.split("_").collect::<Vec<_>>()[2]) {
                        bank.insert(item, reply.value.as_i64().unwrap() as i32);
                    }
                }
                Ok(())
            }
        },
        Err(ArchipelagoError::NetworkError(err)) => {
            log::info!("Failed to receive data, reconnecting: {}", err);
            let data = ui::get_login_data().lock()?;
            *client = connect_archipelago(ArchipelagoConnection {
                url: data.archipelago_url.clone(),
                name: data.username.clone(),
                password: data.username.clone(),
            })
                .await?;
            Ok(())
        }
        Err(ArchipelagoError::IllegalResponse { received, expected }) => {
            log::error!(
                "Illegal response, expected {:#?}, got {:?}",
                expected,
                received
            );
            Err(ArchipelagoError::IllegalResponse { received, expected }.into())
        }
        Err(ArchipelagoError::ConnectionClosed) => {
            CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::Relaxed);
            log::info!("Connection closed"); // TODO Update status?
            Err(ArchipelagoError::ConnectionClosed.into())
        }
        Err(ArchipelagoError::FailedSerialize(err)) => {
            log::error!("Failed to serialize message: {}", err);
            Err(ArchipelagoError::FailedSerialize(err).into())
        }
        Err(ArchipelagoError::NonTextWebsocketResult(msg)) => {
            log::error!("Non-text websocket result: {:?}", msg);
            Err(ArchipelagoError::NonTextWebsocketResult(msg).into())
        }
    }
}

async fn handle_bounced(
    bounced: Bounced,
    _client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn Error>> {
    const DEATH_LINK: &str = "Deathlink";
    if bounced.tags.contains(&DEATH_LINK.to_string()) {
        log::debug!("Deathlink detected");
        log::info!("{}", bounced.data.get("cause").unwrap().as_str().unwrap());
        kill_dante();
    }
    Ok(())
}

pub(crate) fn kill_dante() {
    let char_data_ptr: usize =
        read_data_from_address(*DMC3_ADDRESS.read().unwrap() + utilities::ACTIVE_CHAR_DATA);
    unsafe {
        write_unaligned((char_data_ptr + 0x411C) as *mut f32, 0.0);
    }
}

fn handle_retrieved(retrieved: Retrieved) -> Result<(), Box<dyn Error>> {
    let mut bank = get_bank().write()?;
    bank.iter_mut().for_each(|(item_name, count)| {
        log::debug!("Reading {}", item_name);
        *count = retrieved
            .keys
            .get(get_bank_key(item_name))
            .unwrap()
            .as_i64()
            .unwrap_or_default() as i32;
        log::debug!("Set count {}", item_name);
    });
    Ok(())
}

pub fn set_data_package(value: DataPackageObject) {
    let mut lock = DATA_PACKAGE.write().unwrap();
    *lock = Some(value);
}

pub fn get_data_package() -> Option<RwLockReadGuard<'static, Option<DataPackageObject>>> {
    let guard = DATA_PACKAGE.read().unwrap();
    if guard.is_some() {
        Some(guard) // caller will need to deref and unwrap
    } else {
        None
    }
}

pub fn get_location_item_name(received_item: &Location) -> Result<&'static str, anyhow::Error> {
    let Ok(mapping_data) = mapping::MAPPING.read() else {
        return Err(anyhow!("Unable to get mapping data"));
    };
    let Some(mapping_data) = mapping_data.as_ref() else {
        return Err(anyhow!("No mapping data"));
    };
    for (location_key, item_entry) in generated_locations::ITEM_MISSION_MAP.iter() {
        //log::debug!("Checking room {} vs {} and mission {} vs {}", item_entry.room_number as i32, received_item.room, item_entry.mission as i32, received_item._mission);
        if item_entry.room_number as i32 == received_item.room {
            if item_entry.x_coord == 0
                || (item_entry.x_coord == received_item.x_coord
                && item_entry.y_coord == received_item.y_coord
                && item_entry.z_coord == received_item.z_coord)
            {
                let location_data = mapping_data.items.get(*location_key).unwrap();
                log::debug!("Believe this to be: {}", location_key);
                log::debug!(
                    "Checking location items: {:#X} vs {:#X}",
                    constants::get_item_id(&*location_data.item_name).unwrap(),
                    received_item.item_id as u8
                );
                if constants::get_item_id(&*location_data.item_name).unwrap()
                    == received_item.item_id as u8
                {
                    // Then see if the item picked up matches the specified in the map
                    return Ok(location_key);
                } else if received_item.item_id == *ITEM_ID_MAP.get("Remote").unwrap() as u64 {
                    // TODO This may be stupid
                    return Ok(location_key);
                }
            }
        }
    }
    Err(anyhow!("No location found"))
}

async fn handle_item_receive(
    client: &mut ArchipelagoClient,
    received_item: Location,
) -> Result<(), Box<dyn Error>> {
    // See if there's an item!
    log::info!("Processing item: {}", received_item);
    let Ok(mapping_data) = mapping::MAPPING.read() else {
        return Err(Box::from(anyhow!("Unable to get mapping data")));
    };
    let Some(mapping_data) = mapping_data.as_ref() else {
        return Err(Box::from(anyhow!("No mapping data")));
    };
    if let Some(data_guard) = get_data_package() {
        if let Some(data) = data_guard.as_ref() {
            let location_key = get_location_item_name(&received_item)?;
            let location_data = mapping_data.items.get(location_key).unwrap();
            // Then see if the item picked up matches the specified in the map
            match data
                .games
                .get(GAME_NAME)
                .unwrap()
                .location_name_to_id
                .get(location_key)
            {
                Some(loc_id) => {
                    edit_end_event(&location_key);
                    let desc = location_data.description.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(15)).await;
                        text_handler::display_message_via_index(desc);
                    });
                    text_handler::CANCEL_TEXT.store(true, Ordering::Relaxed);
                    if let Err(arch_err) = client.location_checks(vec![loc_id.clone()]).await {
                        log::error!("Failed to check location: {}", arch_err);
                        item_sync::add_offline_check(loc_id.clone(), client).await?;
                    }
                    if constants::get_items_by_category(ItemCategory::Key)
                        .contains(&&*location_data.item_name)
                    {
                        ui::set_checklist_item(&*location_data.item_name, true);
                        log::debug!("Key Item checked off: {}", location_data.item_name);
                    }
                    log::info!(
                        "Location check successful: {} ({}), Item: {}",
                        location_key,
                        loc_id,
                        location_data.item_name
                    );
                }
                None => Err(anyhow::anyhow!("Location not found: {}", location_key))?,
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
                                utilities::replace_single_byte(
                                    constants::EVENT_TABLE_ADDR + event.offset,
                                    0x00, // (TODO) NOTE: This will fail if something like DDMK's arcade mode is used, due to the player having no officially picked up red orbs. But this shouldn't occur in normal gameplay.
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
    // TODO Can I consolidate this down?
    //log::debug!("Printing json: {:?}", print_json);
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
        PrintJSON::Part {
            data,
            team: _team,
            slot: _slot,
        } => {
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
        PrintJSON::ServerChat {
            data,
            message: _message,
        } => {
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
        PrintJSON::Goal {
            data,
            team: _team,
            slot: _slot,
        } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Release {
            data,
            team: _team,
            slot: _slot,
        } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Collect {
            data,
            team: _team,
            slot: _slot,
        } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Countdown {
            data,
            countdown: _countdown,
        } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
        PrintJSON::Text { data } => {
            for message in data {
                final_message.push_str(&*handle_message_part(message));
            }
        }
    }
    final_message
}

fn handle_message_part(message: JSONMessagePart) -> String {
    match message {
        JSONMessagePart::PlayerId { text } => get_connected().lock().unwrap().players
            [text.parse::<usize>().unwrap() - 1]
            .name
            .clone(),
        JSONMessagePart::PlayerName { text } => text,
        JSONMessagePart::ItemId {
            text,
            flags: _flags,
            player,
        } => match read_cache() {
            Ok(cache) => {
                let id_to_name: HashMap<i64, String> = cache
                    .games
                    .get(
                        &get_connected().lock().unwrap().slot_info[&player.to_string()]
                            .game
                            .clone(),
                    )
                    .unwrap()
                    .item_name_to_id
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (v, k))
                    .collect();
                id_to_name
                    .get(&text.parse::<i64>().unwrap())
                    .unwrap()
                    .clone()
            }
            Err(err) => format!("Unable to read cache file: {}", err),
        },
        JSONMessagePart::ItemName {
            text,
            flags,
            player,
        } => {
            log::debug!("ItemName: {:?} Flags: {}, Player: {}", text, flags, player);
            text
        }
        JSONMessagePart::LocationId { text, player } => match read_cache() {
            Ok(cache) => {
                let id_to_name: HashMap<i64, String> = cache
                    .games
                    .get(
                        &get_connected().lock().unwrap().slot_info[&player.to_string()]
                            .game
                            .clone(),
                    )
                    .unwrap()
                    .location_name_to_id
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (v, k))
                    .collect();
                id_to_name
                    .get(&text.parse::<i64>().unwrap())
                    .unwrap()
                    .clone()
            }
            Err(err) => format!("Unable to read cache file: {}", err),
        },
        JSONMessagePart::LocationName { text, player } => {
            log::debug!("LocationName: {:?}, Player: {}", text, player);
            text
        }
        JSONMessagePart::EntranceName { text } => text,
        JSONMessagePart::Color { text, color } => {
            match color {
                // This looks ugly, but I'm too lazy to have a better idea
                JSONColor::Bold => text.bold().to_string(),
                JSONColor::Underline => text.underline().to_string(),
                JSONColor::Black => text.black().to_string(),
                JSONColor::Red => text.red().to_string(),
                JSONColor::Green => text.green().to_string(),
                JSONColor::Yellow => text.yellow().to_string(),
                JSONColor::Blue => text.blue().to_string(),
                JSONColor::Magenta => text.magenta().to_string(),
                JSONColor::Cyan => text.cyan().to_string(),
                JSONColor::White => text.white().to_string(),
                JSONColor::BlackBg => text.on_black().to_string(),
                JSONColor::RedBg => text.on_red().to_string(),
                JSONColor::GreenBg => text.on_green().to_string(),
                JSONColor::YellowBg => text.on_yellow().to_string(),
                JSONColor::BlueBg => text.on_blue().to_string(),
                JSONColor::MagentaBg => text.on_magenta().to_string(),
                JSONColor::CyanBg => text.on_cyan().to_string(),
                JSONColor::WhiteBg => text.on_white().to_string(),
            }
        }
        JSONMessagePart::Text { text } => text,
    }
}
