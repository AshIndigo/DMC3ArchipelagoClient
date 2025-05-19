use crate::cache::read_cache;
use crate::constants::GAME_NAME;
use crate::hook::CLIENT;
use crate::ui::ui;
use crate::ui::ui::CHECKLIST;
use crate::{archipelago, bank};
use archipelago_rs::client::ArchipelagoClient;
use archipelago_rs::protocol::{NetworkItem, ReceivedItems};
use log;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

const SYNC_FILE: &str = "archipelago.json";
pub(crate) static SYNC_DATA: OnceLock<Mutex<SyncData>> = OnceLock::new();
pub(crate) static CURRENT_INDEX: AtomicI32 = AtomicI32::new(0);

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct SyncData {
    pub sync_indices: HashMap<String, i32>,
}

pub fn get_sync_data() -> &'static Mutex<SyncData> {
    SYNC_DATA.get_or_init(|| Mutex::new(SyncData::default()))
}

pub async fn write_sync_data() -> Result<(), Box<dyn Error>> {
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
    // READ https://github.com/ArchipelagoMW/Archipelago/blob/main/docs/network%20protocol.md#synchronizing-items
    // Handle Checklist items here
    let id_to_name: HashMap<i64, String> = read_cache()?
        .games
        .get(GAME_NAME)
        .unwrap()
        .item_name_to_id
        .clone()
        .into_iter()
        .map(|(k, v)| (v, k))
        .collect();
    for item in &received_items_packet.items {
        ui::set_checklist_item(id_to_name.get(&item.item).unwrap(), true);
    }
    log::debug!("Received Items Packet: {:?}", received_items_packet);
    /*
    So maybe add all NetworkItems to SyncData as well? Then compare with packet?
     */
    if received_items_packet.index == 0
        || received_items_packet.index > CURRENT_INDEX.load(Ordering::SeqCst)
    {
        for item in &received_items_packet.items {
            /*    unsafe { // TODO This will crash due to a bug with display_message (Needs another "vanilla" message to prep)
                utilities::display_message(format!(
                    "Received {}!",
                    constants::get_item(item.item as u64)
                ));
            }*/
            if item.item < 0x14 {
                // TODO Bank stuff broken
                // Consumables/orbs TODO
                if let Some(tx) = bank::TX_BANK_ADD.get() {
                    tx.send(NetworkItem {
                        item: item.item,
                        location: item.location,
                        player: item.player,
                        flags: item.flags,
                    })
                    .await?;
                }
                //bank::add_item(client, item).await; // TODO Somethings fucked with this
            }
        }
        log::debug!("storing data");
        CURRENT_INDEX.store(received_items_packet.index, Ordering::SeqCst);
        get_sync_data()
            .lock()
            .unwrap()
            .sync_indices
            .insert(get_index(client), received_items_packet.index);
        log::debug!("data stored");
    }

    log::debug!("Writing sync file");
    write_sync_data().await?;
    Ok(())
}

pub async fn handle_received_items(received_items: ReceivedItems) -> Result<(), Box<dyn Error>> {
    let mut client_lock = CLIENT.lock().await;
    if let Some(ref mut client) = client_lock.as_mut() {
        handle_received_items_packet(received_items, client).await
    } else {
        Err("Failed to get client lock".into())
    }
}

pub(crate) fn get_index(cl: &ArchipelagoClient) -> String {
    format!(
        "{}_{}",
        cl.room_info().seed_name,
        archipelago::get_connected().lock().unwrap().slot
    )
}

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
pub(crate) async fn sync_items() {
    let mut client_lock = CLIENT.lock().await;
    if let Some(ref mut client) = client_lock.as_mut() {
        log::info!("Synchronizing items");
        CHECKLIST.get().unwrap().write().unwrap().clear();
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
