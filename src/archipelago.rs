use crate::bank::{get_bank, get_bank_key};
use crate::check_handler::{Location, LocationType};
use crate::connection_manager::CONNECTION_STATUS;
use crate::constants::{get_item_name, ItemCategory, GAME_NAME, MISSION_ITEM_MAP, REMOTE_ID};
use crate::game_manager::{get_mission, ArchipelagoData, Style, ARCHIPELAGO_DATA};
use crate::mapping::{DeathlinkSetting, Goal, Mapping, MAPPING};
use crate::ui::font_handler::{WHITE, YELLOW};
use crate::ui::overlay::{MessageSegment, MessageType, OverlayMessage};
use crate::ui::{overlay, text_handler};
use crate::{bank, constants, game_manager, hook, location_handler, mapping, skill_manager};
use randomizer_utilities::item_sync::{get_index, RoomSyncInfo, CURRENT_INDEX};

use randomizer_utilities::ui_utilities::Status;

use archipelago_rs::{
    AsItemId, Client, ClientStatus, Connection, ConnectionOptions, CreateAsHint, Event, Item,
    ItemHandling, ReceivedItem, UpdatedField,
};
use randomizer_utilities::archipelago_utilities::{handle_print, DeathLinkData};
use randomizer_utilities::item_sync;
use serde_json::Value;
use std::cmp::PartialEq;
use std::error::Error;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub static TX_CONNECT: OnceLock<Sender<String>> = OnceLock::new();
pub static TX_DISCONNECT: OnceLock<Sender<bool>> = OnceLock::new();

// May as well make this a struct so I can easily tack on more later
pub struct ArchipelagoCore {
    pub connection: Connection<Value>,
}

impl ArchipelagoCore {
    pub fn new(url: String, game_name: String) -> anyhow::Result<Self> {
        Ok(Self {
            connection: Connection::new(
                url,
                game_name,
                "",
                ConnectionOptions::new().receive_items(ItemHandling::OtherWorlds {
                    own_world: true,
                    starting_inventory: true,
                }),
            ),
        })
    }

    pub fn update(&mut self) -> Result<(), Box<dyn Error>> {
        for event in self.connection.update() {
            match event {
                Event::Connected => {
                    log::info!("Connected!");
                }
                Event::Updated(upt) => {
                    for fld in upt {
                        match fld {
                            UpdatedField::ServerTags(_) => {}
                            UpdatedField::Permissions { .. } => {}
                            UpdatedField::HintEconomy { .. } => {}
                            UpdatedField::HintPoints(_) => {}
                            UpdatedField::Players(_) => {}
                            UpdatedField::CheckedLocations(_) => {}
                        }
                    }
                }
                Event::Print(print) => {
                    let str = handle_print(print);
                    log::info!("Print from server: {}", str);
                }
                Event::ReceivedItems(idx) => {
                    handle_received_items_packet(idx, self.connection.client_mut().unwrap())?;
                }
                Event::Error(err) => log::error!("{}", err),
                Event::Bounce { .. } => {}
                Event::DeathLink {
                    games,
                    slots,
                    tags,
                    time,
                    cause,
                    source,
                } => {
                    overlay::add_message(OverlayMessage::new(
                        vec![MessageSegment::new(
                            format!("{}: {}", source, cause.unwrap_or_default()),
                            WHITE,
                        )],
                        Duration::from_secs(3),
                        // TODO May want to adjust position, currently added to the 'notification list' so it's in the upper right queue
                        0.0,
                        0.0,
                        MessageType::Notification,
                    ));
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
                Event::KeyChanged {
                    key,
                    old_value,
                    new_value,
                    player,
                } => {}
            }
        }
        Ok(())
    }
}

/// This is run when a there is a valid connection to a room.
pub async fn run_setup(cl: &mut Client) -> Result<(), Box<dyn Error>> {
    log::info!("Running setup");
    hook::rewrite_mode_table();
    // match cl.client_mut().unwrap().this_game(). {
    //     // Set the data package global based on received or cached values
    //     Some(data_package) => {
    //         log::info!("Using received data package");
    //         cache::set_data_package(data_package.clone())?;
    //     }
    //     None => {
    //         log::info!("No data package data received, using cached data");
    //         cache::set_data_package(read_cache()?)?;
    //     }
    //}

    //update_checked_locations()?;

    let mut sync_data = item_sync::get_sync_data().lock()?;
    *sync_data = item_sync::read_save_data().unwrap_or_default();
    let index = get_index(
        cl.seed_name(),
        cl.this_player().slot(), //&cl.room_info().seed_name,
                                 //SLOT_NUMBER.load(Ordering::SeqCst),
    );
    if sync_data.room_sync_info.contains_key(&index) {
        CURRENT_INDEX.store(
            sync_data.room_sync_info.get(&index).unwrap().sync_index,
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
                log::debug!("Mapping data: {:#?}", MAPPING.read()?);
            }
        }
        Err(err) => {
            return Err(
                format!("Failed to load mappings from slot data, aborting: {}", err).into(),
            );
        }
    }
    mapping::use_mappings(cl)?;
    Ok(())
}

pub static TX_DEATHLINK: OnceLock<Sender<DeathLinkData>> = OnceLock::new();

pub async fn handle_things(
    client: &mut Connection,
    loc_rx: &mut Receiver<Location>,
    bank_rx: &mut Receiver<(&'static str, i32)>,
    connect_rx: &mut Receiver<String>,
    deathlink_rx: &mut Receiver<DeathLinkData>,
    disconnect_request: &mut Receiver<bool>,
) {
    loop {
        //tokio::select! {
        /*            Some(message) = loc_rx.recv() => {
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
            // if let Err(err) = send_deathlink_message(client, message).await {
            //     log::error!("Failed to send deathlink: {}", err);
            // }
                if let Err(err) = client.death_link(get_own_slot_name().unwrap_or_default(), DeathLinkOptions::new().cause(message.cause)).await {
                log::error!("Failed to send deathlink: {}", err);
            }
        }
        Some(reconnect_request) = connect_rx.recv() => {
            log::warn!("Reconnect requested while connected: {}", reconnect_request);
            break; // Exit to trigger reconnect in spawn_arch_thread
        }
        Some(_disconnect_request) = disconnect_request.recv() => {
            disconnect().await;
            break;
        }
        message = client.recv() => {
            if let Err(err) = handle_client_messages(message, client).await {
                log::error!("Client error or disconnect: {}", err);
                break; // Exit to allow clean reconnect
            }
        }*/
        //}
    }
}

async fn disconnect() {
    // TODO I want this to actually be useful. Need to make sure its called when I disconnect on the proxy
    log::info!("Disconnecting and restoring game");
    CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::Relaxed);
    // TEAM_NUMBER.store(-1, Ordering::SeqCst);
    // SLOT_NUMBER.store(-1, Ordering::SeqCst);
    match hook::disable_hooks() {
        Ok(_) => {
            log::debug!("Disabled hooks");
        }
        Err(e) => {
            log::error!("Failed to disable hooks: {:?}", e);
        }
    }
    MAPPING.write().unwrap().take(); // Clear mappings
    *ARCHIPELAGO_DATA.write().unwrap() = ArchipelagoData::default(); // Reset Data (Probably not needed
    // Do I need to something for the bank here?
    hook::restore_item_table();
    hook::restore_mode_table();
    log::info!("Game restored to default state");
}

/*async fn handle_client_messages(
    result: Result<Option<ServerMessage<Value>>, ArchipelagoError>,
    client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn Error>> {
    match result {
        Ok(opt_msg) => match opt_msg {
            None => Ok(()),
            Some(ServerMessage::RichPrint(json_msg)) => {
                match CONNECTED.read().as_ref() {
                    Ok(fuck) => {
                        archipelago_utilities::handle_print_json(json_msg, fuck);
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
            Some(ServerMessage::Bounced(bounced_msg)) => handle_bounced(bounced_msg).await,
            Some(ServerMessage::InvalidPacket(invalid_packet)) => {
                log::error!("Invalid packet: {:?}", invalid_packet);
                Ok(())
            }
            Some(ServerMessage::Retrieved(retrieved)) => handle_retrieved(retrieved),
            Some(ServerMessage::SetReply(reply)) => {
                log::debug!("SetReply: {:?}", reply);
                let mut bank = get_bank().write()?;
                for item in constants::get_items_by_category(ItemCategory::Consumable).iter() {
                    if item.eq(&reply.key.split("_").collect::<Vec<_>>()[2]) {
                        bank.insert(item, reply.value.as_i64().unwrap() as i32);
                    }
                }
                Ok(())
            }
        },
    }
}*/

// TODO Figure this out
/*fn handle_retrieved(retrieved: Retrieved) -> Result<(), Box<dyn Error>> {
    let mut bank = get_bank().write()?;
    bank.iter_mut().for_each(|(item_name, count)| {
        //log::debug!("Reading {}", item_name);
        match retrieved.keys.get(get_bank_key(item_name)) {
            None => {
                log::error!("Bank key: {} not found", item_name);
            }
            Some(cnt) => *count = cnt.as_i64().unwrap_or_default() as i32,
        }
        //log::debug!("Set count {}", item_name);
    });
    Ok(())
}
*/
async fn handle_item_receive(
    client: &mut Client,
    received_item: Location,
) -> Result<(), Box<dyn Error>> {
    // See if there's an item!
    log::info!("Processing item: {}", received_item);
    if let Some(mapping_data) = MAPPING.read()?.as_ref() {
        if received_item.location_type == LocationType::Standard && received_item.item_id <= 0x39 {
            crate::check_handler::take_away_received_item(received_item.item_id);
        }
        let location_key = location_handler::get_location_name_by_data(&received_item)?;
        // TODO Fix this up
        //let location_data = mapping_data.items.get(location_key).unwrap();
        // Then see if the item picked up matches the specified in the map
        match client
            .this_game()
            .location_by_name(location_key.to_string())
        {
            Some(loc_id) => {
                location_handler::edit_end_event(location_key); // Needed so a mission will end properly after picking up its trigger.
                text_handler::replace_unused_with_text(location_data.get_description()?);
                text_handler::CANCEL_TEXT.store(true, Ordering::SeqCst);
                if let Err(arch_err) = client.location_checks(vec![*loc_id]).await {
                    log::error!("Failed to check location: {}", arch_err);
                    let index = get_index(&client.seed_name(), client.this_player().slot());
                    item_sync::add_offline_check(*loc_id, index).await?;
                }
                let name = location_data.get_item_name()?;
                if let Ok(mut archipelago_data) = ARCHIPELAGO_DATA.write()
                    && location_data.get_in_game_id::<constants::DMC3Config>() > 0x14
                    && location_data.get_in_game_id::<constants::DMC3Config>() != *REMOTE_ID
                {
                    archipelago_data.add_item(get_item_name(
                        location_data.get_in_game_id::<constants::DMC3Config>(),
                    ));
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
        if has_reached_goal(client, &mapping_data) {
            client.set_status(ClientStatus::Goal)?
        }
    }

    Ok(())
}

fn has_reached_goal(client: &mut Client, mapping: &&Mapping) -> bool {
    let mut chk = client.checked_locations();
    match mapping.goal {
        Goal::Standard => chk
            .find(|loc| loc.name() == &"Mission #20 Complete")
            .is_some(),
        Goal::All => {
            for i in 1..20 {
                // If we are missing a mission complete check then we cannot goal
                if !chk
                    .find(|loc| loc.name() == &format!("Mission #{} Complete", i).as_str())
                    .is_some()
                {
                    return false;
                }
            }
            // If we have them all, goal
            true
        }
        Goal::RandomOrder => {
            if let Some(order) = &mapping.mission_order {
                return chk
                    .find(|loc| loc.name() == &format!("Mission #{} Complete", order[19]).as_str())
                    .is_some();
            }
            false
        }
    }
}

pub fn handle_received_items_packet(
    index: usize,
    client: &mut Client,
) -> Result<(), Box<dyn Error>> {
    // Handle Checklist items here
    *item_sync::get_sync_data()
        .lock()
        .expect("Failed to get Sync Data") = item_sync::read_save_data().unwrap_or_default();

    CURRENT_INDEX.store(
        item_sync::get_sync_data()
            .lock()?
            .room_sync_info
            .get(&get_index(&client.seed_name(), client.this_player().slot()))
            .unwrap_or(&RoomSyncInfo::default())
            .sync_index,
        Ordering::SeqCst,
    );

    if index == 0 {
        // If 0 abandon previous inv.
        bank::read_values(client)?;
        match ARCHIPELAGO_DATA.write() {
            Ok(mut data) => {
                *data = ArchipelagoData::default();
                skill_manager::reset_expertise();
                for item in client.received_items() {
                    match item.item().as_item_id() {
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
                    if item.item().as_item_id() < 0x53 && item.item().as_item_id() > 0x39 {
                        skill_manager::add_skill(item.item().as_item_id() as usize, &mut data);
                    }
                }
            }
            Err(err) => {
                log::error!("Couldn't get ArchipelagoData for write: {}", err)
            }
        }
    }
    if let Some(item) = client
        .received_items()
        .iter()
        .find(|item| item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize)
    {
        log::debug!("Received new item: {:?}", item);
        match ARCHIPELAGO_DATA.write() {
            Ok(mut data) => {
                let rec_msg: Vec<MessageSegment> = vec![
                    MessageSegment::new("Received ".to_string(), WHITE),
                    MessageSegment::new(
                        item.item().name().to_string(),
                        overlay::get_color_for_item(item.as_ref()),
                    ),
                    MessageSegment::new(" from ".to_string(), WHITE),
                    MessageSegment::new(item.sender().name().parse()?, YELLOW),
                ];
                overlay::add_message(OverlayMessage::new(
                    rec_msg,
                    Duration::from_secs(3),
                    0.0,
                    0.0,
                    MessageType::Notification,
                ));
                if item.item().as_item_id() < 0x14
                    && let Some(tx) = bank::TX_BANK_MESSAGE.get()
                {
                    tx.send((item.item().name().into(), 1))?;
                }

                log::debug!("Supplying added HP/Magic if needed");
                match item.item().as_item_id() {
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
                        log::debug!("Non style/gun level id: {}", item.item())
                    }
                }
                // For key items
                if item.item().id() >= 0x24 && item.item().id() <= 0x39 {
                    log::debug!("Setting newly acquired key items");
                    match MISSION_ITEM_MAP.get(&(get_mission())) {
                        None => {} // No items for the mission
                        Some(item_list) => {
                            if item_list.contains(&&*item.item().name()) {
                                game_manager::set_item(&*item.item().name(), true, true);
                            }
                        }
                    }
                }

                if (item.item().id() < 0x53 && item.item().id() > 0x39)
                    && let Some(mapping) = MAPPING.read()?.as_ref()
                    && mapping.randomize_skills
                {
                    skill_manager::add_skill(item.item().id() as usize, &mut data);
                    skill_manager::set_skills(&data); // Hacky...
                }
            }
            Err(err) => {
                log::error!("Couldn't get ArchipelagoData for write: {}", err)
            }
        }

        CURRENT_INDEX.store(index as i64, Ordering::SeqCst);
        let mut sync_data = item_sync::get_sync_data().lock()?;
        // TODO I wanna maybe move this to the save slot instead
        let sync_index = get_index(&client.seed_name(), client.this_player().slot());
        // TODO Look at this tomorrow
        let val = sync_data
            .room_sync_info
            .entry(sync_index)
            .or_insert(RoomSyncInfo::default());
        val.sync_index = index as i64;
    }

    if let Ok(mut archipelago_data) = ARCHIPELAGO_DATA.write() {
        if let Some(item) = client
            .received_items()
            .iter()
            .find(|item| item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize)
        {
            archipelago_data.add_item(item.item().name().into());
        }
        // TODO Remove
        // for item in &received_items_packet.items {
        //     archipelago_data.add_item(get_item_name(item.item as u32));
        // }
    }
    log::debug!("Writing sync file");
    item_sync::write_sync_data_file()?;
    Ok(())
}
