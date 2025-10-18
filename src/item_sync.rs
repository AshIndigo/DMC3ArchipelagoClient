use crate::archipelago::SLOT_NUMBER;
use crate::constants::{get_item_name, MISSION_ITEM_MAP};
use crate::game_manager::{get_mission, Style};
use crate::hook::CLIENT;
use crate::mapping::MAPPING;
use crate::ui::font_handler::{WHITE, YELLOW};
use crate::ui::overlay::{MessageSegment, OverlayMessage};
use crate::ui::{overlay};
use crate::{bank, constants, game_manager, mapping, skill_manager};
use archipelago_rs::client::ArchipelagoClient;
use archipelago_rs::protocol::ReceivedItems;
use log;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const SYNC_FILE: &str = "archipelago.json";
pub(crate) static SYNC_DATA: OnceLock<Mutex<SyncData>> = OnceLock::new();
pub(crate) static CURRENT_INDEX: AtomicI32 = AtomicI32::new(0);

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct SyncData {
    pub room_sync_info: HashMap<String, RoomSyncInfo>, // String is seed
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct RoomSyncInfo {
    pub sync_index: i32,
    pub offline_checks: Vec<i64>,
}

pub fn get_sync_data() -> &'static Mutex<SyncData> {
    SYNC_DATA.get_or_init(|| Mutex::new(SyncData::default()))
}

pub async fn write_sync_data_file() -> Result<(), Box<dyn Error>> {
    let mut file = File::create(SYNC_FILE)?;
    log::debug!("Writing sync file");
    file.write_all(
        serde_json::to_string_pretty(&SYNC_DATA.get().expect("Failed to get sync data"))?
            .as_bytes(),
    )?;
    file.flush()?;
    Ok(())
}

pub fn check_for_sync_file() -> bool {
    Path::new(SYNC_FILE).try_exists().unwrap_or_else(|err| {
        log::info!("Failed to check for sync file: {}", err);
        false
    })
}

/// Reads the received items indices from the save file
pub(crate) fn read_save_data() -> Result<SyncData, Box<dyn Error>> {
    if !check_for_sync_file() {
        Ok(SyncData::default())
    } else {
        let save_data = SyncData::deserialize(&mut serde_json::Deserializer::from_reader(
            BufReader::new(File::open(SYNC_FILE)?),
        ))?;
        Ok(save_data)
    }
}

pub(crate) async fn handle_received_items_packet(
    received_items_packet: ReceivedItems,
    client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn Error>> {
    // Handle Checklist items here
    *get_sync_data().lock().expect("Failed to get Sync Data") =
        read_save_data().unwrap_or_default();

    CURRENT_INDEX.store(
        get_sync_data()
            .lock()
            .unwrap()
            .room_sync_info
            .get(&get_index(&client))
            .unwrap_or(&RoomSyncInfo::default())
            .sync_index,
        Ordering::SeqCst,
    );

    if received_items_packet.index == 0 {
        // If 0 abandon previous inv.
        bank::read_values(client).await?;
        match game_manager::ARCHIPELAGO_DATA.write() {
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
        match game_manager::ARCHIPELAGO_DATA.write() {
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
                        MessageSegment::new(mapping::get_slot_name(item.player)?, YELLOW),
                    ];
                    overlay::add_message(OverlayMessage::new(
                        rec_msg,
                        Duration::from_secs(3),
                        0.0,
                        0.0,
                        overlay::MessageType::Notification,
                    ));
                    if item.item < 0x14 {
                        if let Some(tx) = bank::TX_BANK_MESSAGE.get() {
                            tx.send((item_name, 1)).await?;
                        }
                    }
                    log::debug!("Supplying added HP/Magic if needed");
                    match item.item {
                        0x07 => {
                            data.add_blue_orb();
                            game_manager::give_hp(constants::ONE_ORB);
                        }
                        0x08 => {
                            data.add_purple_orb();
                            game_manager::give_magic(constants::ONE_ORB, &*data);
                        }
                        0x19 => {
                            // Awakened Rebellion
                            data.add_dt();
                            game_manager::give_magic(constants::ONE_ORB * 3.0, &*data);
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
                    if item.item < 0x53 && item.item > 0x39 {
                        if let Some(mapping) = MAPPING.read().unwrap().as_ref() {
                            if mapping.randomize_skills {
                                skill_manager::add_skill(item.item as usize, &mut data);
                                skill_manager::set_skills(&data); // Hacky...
                            }
                        }
                    }
                }
            }
            Err(err) => {
                log::error!("Couldn't get ArchipelagoData for write: {}", err)
            }
        }

        CURRENT_INDEX.store(received_items_packet.index, Ordering::SeqCst);
        let mut sync_data = get_sync_data().lock().unwrap();
        if sync_data.room_sync_info.contains_key(&get_index(client)) {
            sync_data
                .room_sync_info
                .get_mut(&get_index(client))
                .unwrap()
                .sync_index = received_items_packet.index;
        } else {
            sync_data
                .room_sync_info
                .insert(get_index(client), RoomSyncInfo::default());
        }
    }
    if let Ok(mut archipelago_data) = game_manager::ARCHIPELAGO_DATA.write() {
        for item in &received_items_packet.items {
            archipelago_data.add_item(get_item_name(item.item as u32));
        }
    }
    log::debug!("Writing sync file");
    write_sync_data_file().await?;
    Ok(())
}

pub(crate) fn get_index(cl: &ArchipelagoClient) -> String {
    format!(
        "{}_{}",
        cl.room_info().seed_name,
        SLOT_NUMBER.load(Ordering::SeqCst)
    )
}

// TODO May remove
#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
pub(crate) async fn _sync_items() {
    if let Some(ref mut client) = CLIENT.lock().await.as_mut() {
        log::info!("Synchronizing items");
        match client.sync().await {
            Ok(received_items) => {
                match handle_received_items_packet(received_items, client).await {
                    Ok(_) => {}
                    Err(err) => {
                        log::error!("Failed to sync items: {}", err);
                    }
                }
            }
            Err(err) => {
                log::error!("Failed to sync items: {}", err);
            }
        }
    }
}

/// Adds an offline location to be sent when room connection is restored
pub(crate) async fn add_offline_check(
    location: i64,
    client: &ArchipelagoClient,
) -> Result<(), Box<dyn Error>> {
    let mut sync_data = get_sync_data().lock()?;
    if sync_data.room_sync_info.contains_key(&get_index(client)) {
        sync_data
            .room_sync_info
            .get_mut(&get_index(client))
            .unwrap()
            .offline_checks
            .push(location);
    } else {
        sync_data
            .room_sync_info
            .insert(get_index(client), RoomSyncInfo::default());
    }
    write_sync_data_file().await?;
    Ok(())
}

pub(crate) async fn send_offline_checks(
    client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn Error>> {
    log::debug!("Attempting to send offline checks");
    let mut sync_data = get_sync_data().lock()?;
    let index = &get_index(client);
    if sync_data.room_sync_info.contains_key(index) {
        match client
            .location_checks(
                sync_data
                    .room_sync_info
                    .get(index)
                    .unwrap()
                    .offline_checks
                    .clone(),
            )
            .await
        {
            Ok(_) => {
                log::info!("Successfully sent offline checks");
                sync_data
                    .room_sync_info
                    .get_mut(index)
                    .unwrap()
                    .offline_checks
                    .clear();
                write_sync_data_file().await?;
            }
            Err(err) => {
                log::error!(
                    "Failed to send offline checks, will attempt next reconnection: {}",
                    err
                );
            }
        }
    }
    Ok(())
}