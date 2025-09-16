use crate::archipelago::{SLOT_NUMBER, TEAM_NUMBER};
use crate::constants::ItemCategory;
use crate::utilities::{get_inv_address, replace_single_byte};
use crate::utilities::{read_data_from_address, DMC3_ADDRESS};
use crate::{constants, create_hook, utilities};
use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use archipelago_rs::protocol::{ClientMessage, DataStorageOperation, Get, NetworkItem, Set};
use minhook::{MinHook, MH_STATUS};
use serde_json::Value;
use std::cmp::max;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, OnceLock, RwLock, RwLockReadGuard};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

pub static BANK: OnceLock<RwLock<HashMap<&'static str, i32>>> = OnceLock::new();
pub static TX_BANK_TO_INV: OnceLock<Sender<(String, i32)>> = OnceLock::new();
pub static TX_BANK_ADD: OnceLock<Sender<NetworkItem>> = OnceLock::new();

pub fn get_bank() -> &'static RwLock<HashMap<&'static str, i32>> {
    BANK.get_or_init(|| {
        RwLock::new(
            constants::get_items_by_category(ItemCategory::Consumable)
                .iter()
                .map(|name| (*name, 0))
                .collect(),
        )
    })
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
pub async fn take_item_from_bank(item_name: &str, count: i32) {
    match TX_BANK_TO_INV.get() {
        None => log::error!("Connect TX doesn't exist"),
        Some(tx) => {
            tx.send((item_name.parse().unwrap(), count))
                .await
                .expect("Failed to send data");
        }
    }
}

pub fn setup_bank_to_inv_channel() -> Receiver<(String, i32)> {
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

pub(crate) async fn add_item_to_bank(
    client: &mut ArchipelagoClient,
    item: &NetworkItem,
) -> Result<(), ArchipelagoError> {
    client
        .send(ClientMessage::Set(Set {
            key: get_bank_key(&constants::get_item_name(item.item as u32)),
            default: Value::from(1),
            want_reply: true,
            operations: vec![DataStorageOperation::Add(Value::from(1))],
        }))
        .await
}

pub(crate) async fn handle_bank(
    client: &mut ArchipelagoClient,
    item: (String, i32),
) -> Result<(), ArchipelagoError> {
    log::debug!("Handling message from bank {:?}", item);
    let bank: RwLockReadGuard<HashMap<&str, i32>> = get_bank().read().unwrap();
    if *bank.get(item.0.as_str()).unwrap() > 0 {
        client
            .send(ClientMessage::Set(Set {
                key: get_bank_key(&item.0),
                default: Value::from(1),
                want_reply: true,
                operations: vec![DataStorageOperation::Add(Value::from(-1 * item.1))],
            }))
            .await?;
        //add_item_to_current_inv(&item.0);
    }
    Ok(())
}

pub(crate) fn can_add_item_to_current_inv(item_name: &str) -> bool {
    let current_inv_addr = get_inv_address();
    if current_inv_addr.is_none() {
        return false;
    }
    let offset = constants::ITEM_OFFSET_MAP
        .get(item_name)
        .unwrap_or_else(|| panic!("Item offset not found: {}", item_name));
    let val = read_data_from_address::<u8>(current_inv_addr.unwrap() + *offset as usize) + 1 // This won't work for red orbs+consumables... int vs byte
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
    if let Some(current_inv_addr) = get_inv_address() {
        let offset = constants::ITEM_OFFSET_MAP
            .get(item_name.as_str())
            .unwrap_or_else(|| panic!("Item offset not found: {}", item_name));
        unsafe {
            replace_single_byte(
                current_inv_addr + *offset as usize,
                read_data_from_address::<u8>(current_inv_addr + *offset as usize) + 1,
            );
        }
    }
}

/// Reset the banks contents to nothing. Used for resetting the values if needed.
pub(crate) async fn _reset_bank(
    client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn std::error::Error>> {
    get_bank().write()?.iter_mut().for_each(|(_k, v)| {
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
    let mut keys = vec![];
    for item in constants::get_items_by_category(ItemCategory::Consumable) {
        keys.push(get_bank_key(item));
    }
    client.send(ClientMessage::Get(Get { keys })).await?;
    Ok(())
}

pub fn setup_bank_hooks() -> Result<(), MH_STATUS> {
    log::debug!("Setting up bank related hooks");
    unsafe {
        create_hook!(
            OPEN_INV_SCREEN_ADDR,
            open_inv_screen,
            ORIGINAL_OPEN_INV_SCREEN,
            "Open Inv Screen"
        );
        create_hook!(
            CLOSE_INV_SCREEN_ADDR,
            close_inv_screen,
            ORIGINAL_CLOSE_INV_SCREEN,
            "Close Inv Screen"
        );
    }
    Ok(())
}

pub unsafe fn disable_bank_hooks(base_address: usize) -> Result<(), MH_STATUS> {
    log::debug!("Disabling bank related hooks");
    unsafe {
        MinHook::disable_hook((base_address + OPEN_INV_SCREEN_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + CLOSE_INV_SCREEN_ADDR) as *mut _)?;
    }
    Ok(())
}

static OPEN_ITEM_COUNT: LazyLock<RwLock<HashMap<&'static str, i32>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub const OPEN_INV_SCREEN_ADDR: usize = 0x87090;
pub static ORIGINAL_OPEN_INV_SCREEN: OnceLock<unsafe extern "C" fn(param_1: usize)> =
    OnceLock::new();

// Game copies the inventory's contents to another part of memory
pub fn open_inv_screen(param_1: usize) {
    // More accurately, this is the whole ass status screen
    unsafe {
        if let Some(orig) = ORIGINAL_OPEN_INV_SCREEN.get() {
            orig(param_1)
        }
    }
    log::debug!("Opening Inv Screen");
    let bank = get_bank().read().unwrap();
    log::debug!("Bank values: {:?}", bank);
    for item in constants::get_items_by_category(ItemCategory::Consumable) {
        let item_address = param_1 + 0xC + constants::get_item_id(item).unwrap() as usize;
        log::debug!(
            "B: {} in screen inv is: {}",
            item,
            read_data_from_address::<u8>(item_address)
        );
        OPEN_ITEM_COUNT
            .write()
            .unwrap()
            .insert(item, read_data_from_address(item_address));
        unsafe {
            replace_single_byte(
                item_address,
                read_data_from_address::<u8>(item_address) + *bank.get(item).unwrap() as u8,
            );
        }
        log::debug!(
            "A: {} in screen inv is: {}",
            item,
            read_data_from_address::<u8>(item_address)
        );
    }
}

pub const CLOSE_INV_SCREEN_ADDR: usize = 0x87460;
pub static ORIGINAL_CLOSE_INV_SCREEN: OnceLock<unsafe extern "C" fn(param_1: usize)> =
    OnceLock::new();

// Upon closing the game copies the contents of the status screen "inv" back to the real inventory
pub fn close_inv_screen(param_1: usize) {
    log::debug!("Closing Inv Screen");
    let bank = get_bank().read().unwrap();
    log::debug!("Bank values: {:?}", bank);
    for item in constants::get_items_by_category(ItemCategory::Consumable) {
        let status_item_address = param_1 + 0xC + constants::get_item_id(item).unwrap() as usize;
        log::debug!("B: {} status addr is {:#X}", item, status_item_address);
        if read_data_from_address::<u8>(status_item_address)
            != *(OPEN_ITEM_COUNT.read().unwrap().get(item).unwrap()) as u8
        {
            let used_item = *(OPEN_ITEM_COUNT.read().unwrap().get(item).unwrap_or(&0))
                - read_data_from_address::<u8>(status_item_address) as i32;
            let to_take_from_bank = *bank.get(item).unwrap() - used_item;
            if to_take_from_bank >= 0 {
                take_item_from_bank(item, to_take_from_bank);
            } else {
                take_item_from_bank(item, *bank.get(item).unwrap());
                unsafe {
                    replace_single_byte(
                        status_item_address,
                        (used_item - (to_take_from_bank.abs())) as u8,
                    )
                }
            }
        }
        unsafe {
            replace_single_byte(
                status_item_address,
                max(
                    read_data_from_address::<u8>(status_item_address)
                        - *bank.get(item).unwrap() as u8,
                    0,
                ), // gotta stop at 0
            );
        }
    }
    unsafe {
        if let Some(orig) = ORIGINAL_CLOSE_INV_SCREEN.get() {
            orig(param_1)
        }
    }
}

// pub const USE_ITEM_ADDR: usize = 0x2affc0;
// pub static ORIGINAL_USE_ITEM: OnceLock<unsafe extern "C" fn(param_1: usize)> =
//     OnceLock::new();
//
// pub fn use_item(param_1: usize) {
//     unsafe {
//         if let Some(orig) = ORIGINAL_USE_ITEM.get() {
//             orig(param_1)
//         }
//     }
// }
