use crate::bank::{get_bank, get_bank_key};
use randomizer_utilities::cache::{read_cache, ChecksumError, DATA_PACKAGE};
use crate::check_handler::Location;
use crate::constants::{get_item_name, ItemCategory, GAME_NAME, MISSION_ITEM_MAP, REMOTE_ID};
use crate::data::generated_locations;
use crate::game_manager::{get_mission, Style, ARCHIPELAGO_DATA};
use crate::mapping::{DeathlinkSetting, Goal, Mapping, MAPPING};
use crate::ui::font_handler::{WHITE, YELLOW};
use crate::ui::overlay::{MessageSegment, MessageType, OverlayMessage};
use crate::ui::{overlay, text_handler};
use crate::connection_manager::CONNECTION_STATUS;
use crate::{bank, constants, game_manager, hook, location_handler, mapping, skill_manager};
use anyhow::anyhow;
use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use archipelago_rs::protocol::{Bounce, Bounced, ClientMessage, ClientStatus, Connected, GetDataPackage, JSONColor, JSONMessagePart, PrintJSON, ReceivedItems, Retrieved, ServerMessage, StatusUpdate};
use owo_colors::OwoColorize;
use serde_json::json;
use std::cmp::PartialEq;
use std::error::Error;
use std::fs::remove_file;
use std::sync::atomic::{Ordering};
use std::sync::{LazyLock, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{Receiver, Sender};
use randomizer_utilities::{cache, item_sync, mapping_utilities};
use randomizer_utilities::archipelago_utilities::{CONNECTED, SLOT_NUMBER, TEAM_NUMBER};
use randomizer_utilities::item_sync::{get_index, RoomSyncInfo, CURRENT_INDEX};
use randomizer_utilities::ui_utilities::Status;

pub static CHECKED_LOCATIONS: LazyLock<RwLock<Vec<&'static str>>> =
    LazyLock::new(|| RwLock::new(vec![]));
pub static TX_CONNECT: OnceLock<Sender<String>> = OnceLock::new();
pub static TX_DISCONNECT: OnceLock<Sender<bool>> = OnceLock::new();


pub async fn get_archipelago_client(
    login_data: &String,
) -> Result<ArchipelagoClient, ArchipelagoError> {
    if cache::check_for_cache_file() {
        // If the cache exists, then connect normally and verify the cache file
        let mut client = ArchipelagoClient::new(&login_data).await?;
        match cache::find_checksum_errors(client.room_info()) {
            Ok(()) => {
                log::info!("Checksums check out!");
                Ok(client)
            }
            Err(err) => {
                match err.downcast::<ChecksumError>() {
                    Ok(checksum_error) => {
                        log::error!(
                            "Local DataPackage checksums for {:?} did not match expected values, reacquiring",
                            checksum_error.games
                        );
                        client
                            .send(ClientMessage::GetDataPackage(GetDataPackage {
                                games: Some(checksum_error.games),
                            }))
                            .await?;
                        match client.recv().await? {
                            Some(ServerMessage::DataPackage(pkg)) => {
                                cache::update_cache(&pkg.data).unwrap_or_else(|err| {
                                    log::error!("Failed to write cache: {}", err)
                                });
                            }
                            Some(received) => {
                                return Err(ArchipelagoError::IllegalResponse {
                                    received,
                                    expected: "DataPackage",
                                });
                            }
                            None => return Err(ArchipelagoError::ConnectionClosed),
                        }
                        return Ok(client);
                    }
                    Err(err) => {
                        log::error!("Error checking DataPackage checksums: {:?}", err);
                        if let Err(err) = remove_file(cache::CACHE_FILENAME) {
                            log::error!("Failed to remove {}: {}", cache::CACHE_FILENAME, err);
                        };
                    }
                }
                Err(ArchipelagoError::ConnectionClosed)
            }
        }
    } else {
        // If the cache file does not exist, then it needs to be acquired
        let cl = ArchipelagoClient::with_data_package(&login_data, None).await?;
        match &cl.data_package() {
            // Write the data package to a local cache file
            None => {
                log::error!("No data package found");
                Err(ArchipelagoError::ConnectionClosed)
            }
            Some(dp) => {
                cache::write_cache(&dp)
                    .await
                    .unwrap_or_else(|err| log::error!("Failed to write cache: {}", err));
                Ok(cl)
            }
        }
    }
}

pub async fn connect_archipelago(
    login_data: String,
) -> Result<ArchipelagoClient, ArchipelagoError> {
    log::info!("Attempting room connection");
    let mut ap_client = get_archipelago_client(&login_data).await?;
    let connected = ap_client
        .connect(
            GAME_NAME,
            "",
            None,
            Option::from(0b111),
            vec!["AP".to_string()],
        )
        .await?;
    match CONNECTED.write() {
        Ok(mut con) => {
            con.replace(connected);
        }
        Err(err) => {
            log::error!("Failed to acquire lock for connection: {}", err);
            return Err(ArchipelagoError::ConnectionClosed);
        }
    }

    match CHECKED_LOCATIONS.write() {
        Ok(mut checked_locations) => {
            checked_locations.clear();
        }
        Err(err) => {
            log::error!("Failed to get checked locations: {}", err);
            return Err(ArchipelagoError::ConnectionClosed);
        }
    }

    let index = get_index(&ap_client.room_info().seed_name, SLOT_NUMBER.load(Ordering::SeqCst));
    item_sync::send_offline_checks(&mut ap_client, index)
        .await
        .unwrap();

    match CONNECTED.read().as_ref() {
        Ok(con) => {
            if let Some(con) = &**con {
                const DEBUG: bool = false;
                if DEBUG {
                    log::debug!("Connected info: {:?}", con);
                }
                SLOT_NUMBER.store(con.slot, Ordering::SeqCst);
                TEAM_NUMBER.store(con.team, Ordering::SeqCst);
            }
        }
        Err(err) => {
            log::error!("Failed to acquire lock for connection: {}", err);
            return Err(ArchipelagoError::ConnectionClosed);
        }
    }

    Ok(ap_client)
}

/// This is run when a there is a valid connection to a room.
pub async fn run_setup(cl: &mut ArchipelagoClient) -> Result<(), Box<dyn Error>> {
    log::info!("Running setup");
    hook::rewrite_mode_table();
    match cl.data_package() {
        // Set the data package global based on received or cached values
        Some(data_package) => {
            log::info!("Using received data package");
            cache::set_data_package(data_package.clone())?;
        }
        None => {
            log::info!("No data package data received, using cached data");
            cache::set_data_package(read_cache()?)?;
        }
    }

    update_checked_locations()?;

    let mut sync_data = item_sync::get_sync_data().lock()?;
    *sync_data = item_sync::read_save_data().unwrap_or_default();
    let index = get_index(&cl.room_info().seed_name, SLOT_NUMBER.load(Ordering::SeqCst));
    if sync_data
        .room_sync_info
        .contains_key(&index)
    {
        CURRENT_INDEX.store(
            sync_data
                .room_sync_info
                .get(&index)
                .unwrap()
                .sync_index,
            Ordering::SeqCst,
        );
    } else {
        CURRENT_INDEX.store(0, Ordering::SeqCst);
    }

    hook::install_initial_functions(); // Hooks needed to modify the game
    match mapping::parse_slot_data() {
        Ok(_) => {
            log::info!("Successfully parsed mapping information");
            const DEBUG: bool = false;
            if DEBUG {
                log::debug!("Mapping data: {:#?}", MAPPING.read().unwrap());
            }
        }
        Err(err) => {
            return Err(
                format!("Failed to load mappings from slot data, aborting: {}", err).into(),
            );
        }
    }
    mapping::use_mappings()?;
    Ok(())
}

fn update_checked_locations() -> Result<(), Box<dyn Error>> {
    log::debug!("Filling out checked locations");
    let dpw_lock = DATA_PACKAGE.read()?;
    let dpw = dpw_lock
        .as_ref()
        .ok_or("DataPackageWrapper was None, this is probably not good")?;

    let mut checked_locations = CHECKED_LOCATIONS.write()?;
    let con_lock = CONNECTED.read()?;
    let con = con_lock.as_ref().ok_or("Connected was None")?;
    let loc_map = dpw
        .location_id_to_name
        .get(GAME_NAME)
        .ok_or(format!("No location_id_to_name entry for {}", GAME_NAME))?;

    for val in &con.checked_locations {
        if let Some(loc_name) = loc_map.get(val) {
            if let Some((key, _)) =
                generated_locations::ITEM_MISSION_MAP.get_key_value(loc_name.as_str())
            {
                checked_locations.push(key);
            }
        }
    }

    Ok(())
}

pub struct DeathLinkData {
    pub cause: String,
}

pub static TX_DEATHLINK: OnceLock<Sender<DeathLinkData>> = OnceLock::new();

pub async fn handle_things(
    client: &mut ArchipelagoClient,
    loc_rx: &mut Receiver<Location>,
    bank_rx: &mut Receiver<(&'static str, i32)>,
    connect_rx: &mut Receiver<String>,
    deathlink_rx: &mut Receiver<DeathLinkData>,
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
                if let Err(err) = bank::modify_bank_value(client, message).await {
                    log::error!("Failed to handle bank: {}", err);
                }
            }
            Some(message) = deathlink_rx.recv() => {
                if let Err(err) = send_deathlink_message(client, message).await {
                    log::error!("Failed to send deathlink: {}", err);
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

async fn send_deathlink_message(
    client: &mut ArchipelagoClient,
    data: DeathLinkData,
) -> Result<(), ArchipelagoError> {
    let name = mapping_utilities::get_own_slot_name().unwrap();
    client
        .send(ClientMessage::Bounce(Bounce {
            games: Some(vec![]),
            slots: Some(vec![]),
            tags: Some(vec![DEATH_LINK.to_string()]),
            data: json!({
                "time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f32(),
                "source": name,
                "cause": data.cause
            }),
        }))
        .await?;
    Ok(())
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
                match CONNECTED.read().as_ref() {
                    Ok(fuck) => {
                        handle_print_json(json_msg, fuck);
                        //&*(*fuck)
                    }
                    Err(err) => {
                        log::error!("Poison Error: {}", err);
                    }
                }
                //log::info!("{}", handle_print_json(json_msg));
                Ok(())
            }
            Some(ServerMessage::RoomInfo(_)) => Ok(()),
            Some(ServerMessage::ConnectionRefused(err)) => {
                CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::Relaxed);
                log::error!("Connection refused: {:?}", err.errors);
                Ok(())
            }
            Some(ServerMessage::Connected(_)) => {
                CONNECTION_STATUS.store(Status::Connected.into(), Ordering::Relaxed);
                Ok(())
            }
            Some(ServerMessage::ReceivedItems(items)) => {
                handle_received_items_packet(items, client).await
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
            CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::Relaxed);
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
            log::info!("Connection closed");
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
const DEATH_LINK: &str = "DeathLink";

async fn handle_bounced(
    bounced: Bounced,
    _client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn Error>> {
    if bounced.tags.is_some() {
        if bounced.tags.unwrap().contains(&DEATH_LINK.to_string()) {
            log::debug!("DeathLink detected");

            // TODO Only display this if in game?
            if bounced.data.is_some() {
                overlay::add_message(OverlayMessage::new(
                    vec![MessageSegment::new(
                        bounced
                            .data
                            .unwrap()
                            .get("cause")
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string(),
                        WHITE,
                    )],
                    Duration::from_secs(3),
                    // TODO May want to adjust position, currently added to the 'notification list' so it's in the upper right queue
                    0.0,
                    0.0,
                    MessageType::Notification,
                ));
            }
            match MAPPING.read()?.as_ref().unwrap().death_link {
                DeathlinkSetting::DeathLink => {
                    game_manager::kill_dante();
                }
                DeathlinkSetting::HurtLink => {
                    game_manager::hurt_dante();
                }
                DeathlinkSetting::Off => {}
            }
        }
    }
    Ok(())
}

fn handle_retrieved(retrieved: Retrieved) -> Result<(), Box<dyn Error>> {
    let mut bank = get_bank().write()?;
    bank.iter_mut().for_each(|(item_name, count)| {
        log::debug!("Reading {}", item_name);
        match retrieved.keys.get(get_bank_key(item_name)) {
            None => {
                log::error!("{} not found", item_name);
            }
            Some(cnt) => *count = cnt.as_i64().unwrap_or_default() as i32,
        }
        log::debug!("Set count {}", item_name);
    });
    Ok(())
}

async fn handle_item_receive(
    client: &mut ArchipelagoClient,
    received_item: Location,
) -> Result<(), Box<dyn Error>> {
    // See if there's an item!
    log::info!("Processing item: {}", received_item);
    let Ok(mapping_data) = MAPPING.read() else {
        return Err(Box::from(anyhow!("Unable to get mapping data")));
    };
    let Some(mapping_data) = mapping_data.as_ref() else {
        return Err(Box::from(anyhow!("No mapping data")));
    };

    if let Some(data_package) = DATA_PACKAGE.read().unwrap().as_ref() {
        if received_item.item_id <= 0x39 {
            crate::check_handler::take_away_received_item(received_item.item_id);
        }
        let location_key = location_handler::get_location_name_by_data(&received_item)?;
        let location_data = mapping_data.items.get(location_key).unwrap();
        // Then see if the item picked up matches the specified in the map
        match data_package
            .dp
            .games
            .get(GAME_NAME)
            .unwrap()
            .location_name_to_id
            .get(location_key)
        {
            Some(loc_id) => {
                location_handler::edit_end_event(location_key); // Needed so a mission will end properly after picking up its trigger.
                text_handler::replace_unused_with_text(location_data.get_description()?);
                text_handler::CANCEL_TEXT.store(true, Ordering::SeqCst);
                if let Err(arch_err) = client.location_checks(vec![*loc_id]).await {
                    log::error!("Failed to check location: {}", arch_err);
                    let index = get_index(&client.room_info().seed_name, SLOT_NUMBER.load(Ordering::SeqCst));
                    item_sync::add_offline_check(*loc_id, index).await?;
                }
                let name = location_data.get_item_name()?;
                if let Ok(mut archipelago_data) = ARCHIPELAGO_DATA.write() {
                    if location_data.get_in_game_id::<constants::DMC3Config>() > 0x14
                        && location_data.get_in_game_id::<constants::DMC3Config>() != *REMOTE_ID
                    {
                        archipelago_data.add_item(get_item_name(location_data.get_in_game_id::<constants::DMC3Config>()));
                    }
                }

                log::info!(
                    "Location check successful: {} ({}), Item: {}",
                    location_key,
                    loc_id,
                    name
                );
            }
            None => Err(anyhow::anyhow!("Location not found: {}", location_key))?,
        }
        // Add to checked locations
        CHECKED_LOCATIONS.write()?.push(location_key);
        if has_reached_goal(&mapping_data) {
            client
                .send(ClientMessage::StatusUpdate(StatusUpdate {
                    status: ClientStatus::ClientGoal,
                }))
                .await?;
            return Ok(());
        }
    }

    Ok(())
}

fn has_reached_goal(mapping: &&Mapping) -> bool {
    if let Ok(chk) = CHECKED_LOCATIONS.read().as_ref() {
        return match mapping.goal {
            Goal::Standard => {
                chk.contains(&"Mission #20 Complete")
            }
            Goal::All => {
                for i in 1..20 {
                    // If we are missing a mission complete check then we cannot goal
                    if !chk.contains(&format!("Mission #{} Complete", i).as_str()) {
                        return false;
                    }
                }
                // If we have them all, goal
                true
            }
            Goal::RandomOrder => {
                if let Some(order) = &mapping.mission_order {
                    return chk.contains(&format!("Mission #{} Complete", order[19]).as_str())
                }
                false
            }
        }
    }
    false
}

fn handle_print_json(print_json: PrintJSON, con_opt: &Option<Connected>) -> String {
    let mut final_message: String = "".to_string();
    match print_json {
        PrintJSON::ItemSend {
            data,
            receiving: _receiving,
            item: _item,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::ItemCheat {
            data,
            receiving: _receiving,
            item: _item,
            team: _team,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::Hint {
            data,
            receiving: _receiving,
            item: _item,
            found: _found,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::Join {
            data,
            team: _team,
            slot: _slot,
            tags: _tags,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::Part {
            data,
            team: _team,
            slot: _slot,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::Chat {
            data,
            team: _team,
            slot: _slot,
            message: _message,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::ServerChat {
            data,
            message: _message,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::Tutorial { data } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::TagsChanged {
            data,
            team: _team,
            slot: _slot,
            tags: _tags,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::CommandResult { data } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::AdminCommandResult { data } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::Goal {
            data,
            team: _team,
            slot: _slot,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::Release {
            data,
            team: _team,
            slot: _slot,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::Collect {
            data,
            team: _team,
            slot: _slot,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::Countdown {
            data,
            countdown: _countdown,
        } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
        PrintJSON::Text { data } => {
            for message in data {
                final_message.push_str(&handle_message_part(message, con_opt));
            }
        }
    }
    final_message
}

fn handle_message_part(message: JSONMessagePart, con_opt: &Option<Connected>) -> String {
    //let con_opt = (&**CONNECTED.read().as_ref().unwrap());
    match message {
        JSONMessagePart::PlayerId { text } => match &con_opt {
            None => "<Connected is None>".to_string(),
            Some(con) => con.players[text.parse::<usize>().unwrap() - 1].name.clone(),
        },
        JSONMessagePart::PlayerName { text } => text,
        JSONMessagePart::ItemId {
            text,
            flags: _flags,
            player,
        } => {
            if let Some(data_package) = DATA_PACKAGE.read().unwrap().as_ref() {
                match con_opt {
                    None => "<Connected is None>".to_string(),
                    Some(con) => {
                        let game = &con.slot_info[&player].game.clone();
                        data_package
                            .item_id_to_name
                            .get(game)
                            .unwrap()
                            .get(&text.parse::<i64>().unwrap())
                            .unwrap()
                            .clone()
                    }
                }
            } else {
                "<Data package unavailable>".parse().unwrap()
            }
        }
        JSONMessagePart::ItemName {
            text,
            flags,
            player,
        } => {
            log::debug!("ItemName: {:?} Flags: {}, Player: {}", text, flags, player);
            text
        }
        JSONMessagePart::LocationId { text, player } => {
            if let Some(data_package) = DATA_PACKAGE.read().unwrap().as_ref() {
                match con_opt {
                    None => "<Connected is None>".to_string(),
                    Some(con) => {
                        let game = &con.slot_info[&player].game.clone();
                        data_package
                            .location_id_to_name
                            .get(game)
                            .unwrap()
                            .get(&text.parse::<i64>().unwrap())
                            .unwrap()
                            .clone()
                    }
                }
            } else {
                "<Data package unavailable>".parse().unwrap()
            }
        }
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

pub(crate) async fn handle_received_items_packet(
    received_items_packet: ReceivedItems,
    client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn Error>> {
    // Handle Checklist items here
    *item_sync::get_sync_data().lock().expect("Failed to get Sync Data") =
        item_sync::read_save_data().unwrap_or_default();

    CURRENT_INDEX.store(
        item_sync::get_sync_data()
            .lock()
            .unwrap()
            .room_sync_info
            .get(&get_index(&client.room_info().seed_name, SLOT_NUMBER.load(Ordering::SeqCst)))
            .unwrap_or(&RoomSyncInfo::default())
            .sync_index,
        Ordering::SeqCst,
    );

    if received_items_packet.index == 0 {
        // If 0 abandon previous inv.
        bank::read_values(client).await?;
        match ARCHIPELAGO_DATA.write() {
            Ok(mut data) => {
                *data = game_manager::ArchipelagoData::default();
                skill_manager::reset_expertise();
                for item in &received_items_packet.items {
                    match item.item {
                        0x07 => {
                            data.add_blue_orb();
                        }
                        0x08 => {
                            data.add_purple_orb();
                        }
                        0x19 => {
                            // Awakened Rebellion
                            data.add_dt();
                        }
                        0x53 => {
                            // Ebony & Ivory
                            data.add_gun_level(0);
                        }
                        0x54 => {
                            // Shotgun
                            data.add_gun_level(1);
                        }
                        0x55 => {
                            // Artemis
                            data.add_gun_level(2);
                        }
                        0x56 => {
                            // Spiral
                            data.add_gun_level(3);
                        }
                        0x57 => {
                            // Kalina Ann
                            data.add_gun_level(4);
                        }
                        0x60 => data.add_style_level(Style::Trickster),
                        0x61 => data.add_style_level(Style::Swordmaster),
                        0x62 => data.add_style_level(Style::Gunslinger),
                        0x63 => data.add_style_level(Style::Royalguard),
                        _ => {}
                    }
                    if item.item < 0x53 && item.item > 0x39 {
                        skill_manager::add_skill(item.item as usize, &mut data);
                    }
                }
            }
            Err(err) => {
                log::error!("Couldn't get ArchipelagoData for write: {}", err)
            }
        }
    }
    if received_items_packet.index > CURRENT_INDEX.load(Ordering::SeqCst) {
        log::debug!("Received new items packet: {:?}", received_items_packet);
        match ARCHIPELAGO_DATA.write() {
            Ok(mut data) => {
                for item in &received_items_packet.items {
                    let item_name = get_item_name(item.item as u32);
                    let rec_msg: Vec<MessageSegment> = vec![
                        MessageSegment::new("Received ".to_string(), WHITE),
                        MessageSegment::new(
                            item_name.to_string(),
                            overlay::get_color_for_item(item.flags),
                        ),
                        MessageSegment::new(" from ".to_string(), WHITE),
                        MessageSegment::new(mapping_utilities::get_slot_name(item.player)?, YELLOW),
                    ];
                    overlay::add_message(OverlayMessage::new(
                        rec_msg,
                        Duration::from_secs(3),
                        0.0,
                        0.0,
                        MessageType::Notification,
                    ));
                    if item.item < 0x14
                        && let Some(tx) = bank::TX_BANK_MESSAGE.get()
                    {
                        tx.send((item_name, 1)).await?;
                    }

                    log::debug!("Supplying added HP/Magic if needed");
                    match item.item {
                        0x07 => {
                            data.add_blue_orb();
                            game_manager::give_hp(constants::ONE_ORB);
                        }
                        0x08 => {
                            data.add_purple_orb();
                            game_manager::give_magic(constants::ONE_ORB, &data);
                        }
                        0x19 => {
                            // Awakened Rebellion
                            data.add_dt();
                            game_manager::give_magic(constants::ONE_ORB * 3.0, &data);
                        }
                        0x53 => {
                            // Ebony & Ivory
                            data.add_gun_level(0);
                        }
                        0x54 => {
                            // Shotgun
                            data.add_gun_level(1);
                        }
                        0x55 => {
                            // Artemis
                            data.add_gun_level(2);
                        }
                        0x56 => {
                            // Spiral
                            data.add_gun_level(3);
                        }
                        0x57 => {
                            // Kalina Ann
                            data.add_gun_level(4);
                        }
                        0x60 => {
                            data.add_style_level(Style::Trickster);
                            game_manager::apply_style_levels(Style::Trickster)
                        }
                        0x61 => {
                            data.add_style_level(Style::Swordmaster);
                            game_manager::apply_style_levels(Style::Swordmaster)
                        }
                        0x62 => {
                            data.add_style_level(Style::Gunslinger);
                            game_manager::apply_style_levels(Style::Gunslinger)
                        }
                        0x63 => {
                            data.add_style_level(Style::Royalguard);
                            game_manager::apply_style_levels(Style::Royalguard)
                        }
                        _ => {
                            log::debug!("Non style/gun level id: {}", item.item)
                        }
                    }
                    // For key items
                    if item.item >= 0x24 && item.item <= 0x39 {
                        log::debug!("Setting newly acquired key items");
                        match MISSION_ITEM_MAP.get(&(get_mission())) {
                            None => {} // No items for the mission
                            Some(item_list) => {
                                if item_list.contains(&item_name) {
                                    game_manager::set_item(item_name, true, true);
                                }
                            }
                        }
                    }

                    if (item.item < 0x53 && item.item > 0x39)
                        && let Some(mapping) = MAPPING.read().unwrap().as_ref()
                        && mapping.randomize_skills
                    {
                        skill_manager::add_skill(item.item as usize, &mut data);
                        skill_manager::set_skills(&data); // Hacky...
                    }
                }
            }
            Err(err) => {
                log::error!("Couldn't get ArchipelagoData for write: {}", err)
            }
        }

        CURRENT_INDEX.store(received_items_packet.index, Ordering::SeqCst);
        let mut sync_data = item_sync::get_sync_data().lock().unwrap();
        let index = get_index(&client.room_info().seed_name, SLOT_NUMBER.load(Ordering::SeqCst));
        if sync_data.room_sync_info.contains_key(&index) {
            sync_data
                .room_sync_info
                .get_mut(&index)
                .unwrap()
                .sync_index = received_items_packet.index;
        } else {
            sync_data
                .room_sync_info
                .insert(index, RoomSyncInfo::default());
        }
    }
    if let Ok(mut archipelago_data) = ARCHIPELAGO_DATA.write() {
        for item in &received_items_packet.items {
            archipelago_data.add_item(get_item_name(item.item as u32));
        }
    }
    log::debug!("Writing sync file");
    item_sync::write_sync_data_file()?;
    Ok(())
}