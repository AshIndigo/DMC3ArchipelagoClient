use crate::archipelago::{SLOT_NUMBER, TEAM_NUMBER};
use crate::constants::{INVENTORY_PTR, ItemCategory};
use crate::{constants, utilities};
use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use archipelago_rs::protocol::{ClientMessage, DataStorageOperation, Get, NetworkItem, Set};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

pub(crate) async fn add_item_to_bank(
    client: &mut ArchipelagoClient,
    item: &NetworkItem,
) -> Result<(), ArchipelagoError> {
    client
        .send(ClientMessage::Set(Set {
            key: get_bank_key(&constants::get_item_name(item.item as u8)),
            default: Value::from(1),
            want_reply: true,
            operations: vec![DataStorageOperation::Add(Value::from(1))],
        }))
        .await
}

pub static TX_BANK_TO_INV: OnceLock<Sender<String>> = OnceLock::new();
pub static TX_BANK_ADD: OnceLock<Sender<NetworkItem>> = OnceLock::new();

pub fn get_bank() -> &'static Mutex<HashMap<&'static str, i32>> {
    BANK.get_or_init(|| {
        Mutex::new(
            constants::get_items_by_category(ItemCategory::Consumable)
                .iter()
                .map(|name| (*name, 0))
                .collect(),
        )
    })
}

pub fn setup_bank_to_inv_channel() -> Receiver<String> {
    let (tx, rx) = mpsc::channel(64);
    TX_BANK_TO_INV.set(tx).expect("TX already initialized");
    rx
}

pub fn setup_bank_add_channel() -> Receiver<NetworkItem> {
    let (tx, rx) = mpsc::channel(64);
    TX_BANK_ADD.set(tx).expect("TX already initialized");
    rx
}

pub(crate) fn get_bank_key(item: &str) -> String {
    format!(
        "team{}_slot{}_{}",
        TEAM_NUMBER.load(Ordering::SeqCst),
        SLOT_NUMBER.load(Ordering::SeqCst),
        item
    )
}

pub(crate) async fn handle_bank(
    client: &mut ArchipelagoClient,
    item: String,
) -> Result<(), ArchipelagoError> {
    log::debug!("Handling message from bank {:?}", item);
    let bank: MutexGuard<HashMap<&str, i32>> = get_bank().lock().unwrap();
    if *bank.get(item.as_str()).unwrap() > 0 {
        client
            .send(ClientMessage::Set(Set {
                key: get_bank_key(&item),
                default: Value::from(1),
                want_reply: true,
                operations: vec![DataStorageOperation::Add(Value::from(-1))],
            }))
            .await?;
        add_item_to_current_inv(&item);
    }
    Ok(())
}

pub(crate) fn can_add_item_to_current_inv(item_name: &str) -> bool {
    let current_inv_addr = utilities::read_usize_from_address(INVENTORY_PTR);
    if current_inv_addr == 0 {
        return false;
    }
    let offset = constants::ITEM_OFFSET_MAP
        .get(item_name)
        .unwrap_or_else(|| panic!("Item offset not found: {}", item_name));
    let val = utilities::read_byte_from_address_no_offset(current_inv_addr + *offset as usize)+1 // This won't work for red orbs+consumables... int vs byte
        < constants::ITEM_MAX_COUNT_MAP
            .get(item_name)
            .unwrap_or_else(|| {
                log::error!("Item does not have a count: {}", item_name);
                &Some(0)
            })
            .unwrap() as u8;
    val
}

pub(crate) fn add_item_to_current_inv(item_name: &String) {
    let current_inv_addr = utilities::read_usize_from_address(INVENTORY_PTR);
    let offset = constants::ITEM_OFFSET_MAP
        .get(item_name.as_str())
        .unwrap_or_else(|| panic!("Item offset not found: {}", item_name));
    unsafe {
        utilities::replace_single_byte_no_offset(
            current_inv_addr + *offset as usize,
            utilities::read_byte_from_address_no_offset(current_inv_addr + *offset as usize) + 1,
        );
    }
}

pub static BANK: OnceLock<Mutex<HashMap<&'static str, i32>>> = OnceLock::new();

/// Reset the banks contents to nothing. Used for resetting the values if needed.
pub(crate) async fn reset_bank(
    client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn std::error::Error>> {
    get_bank().lock()?.iter_mut().for_each(|(_k, v)| {
        *v = 0; // Set each bank item in the map to 0
    });
    for item in constants::get_items_by_category(ItemCategory::Consumable) {
        client
            .send(ClientMessage::Set(Set {
                key: get_bank_key(&item.to_string()),
                default: Value::from(0),
                want_reply: true,
                operations: vec![DataStorageOperation::Default],
            }))
            .await?;
    }
    Ok(())
}

pub(crate) async fn read_values(
    client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO Send out "Get" message, then when reply is received, update bank hashmap with received values
    // ArchipelagoClient#get is annoying
    let mut keys = vec![];
    for item in constants::get_items_by_category(ItemCategory::Consumable) {
        keys.push(get_bank_key(item));
    }
    client.send(ClientMessage::Get(Get { keys })).await?;
    Ok(())
}
