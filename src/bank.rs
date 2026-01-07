use crate::constants::{ItemCategory, ITEM_MAP};
use crate::utilities::{read_data_from_address, DMC3_ADDRESS};
use crate::{constants, create_hook};
use archipelago_rs::{Client, Connection, DataStorageOperation, Error, Player};
use minhook::{MinHook, MH_STATUS};
use randomizer_utilities::replace_single_byte;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{OnceLock, RwLock};
use std::sync::mpsc::Sender;

pub(crate) static BANK: OnceLock<RwLock<HashMap<&'static str, i32>>> = OnceLock::new();
pub static TX_BANK_MESSAGE: OnceLock<Sender<(&'static str, i32)>> = OnceLock::new();

pub(crate) fn get_bank_key(item: &str, player: &Player) -> String {
    format!("team{}_slot{}_{}", player.team(), player.slot(), item)
}

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

pub fn modify_bank_message(item_name: &'static str, count: i32) {
    match TX_BANK_MESSAGE.get() {
        None => log::error!("Connect TX doesn't exist"),
        Some(tx) => {
            tx.send((item_name, count))
                .expect("Failed to send data");
        }
    }
}

pub(crate) fn modify_bank_value(
    connection: &mut Connection,
    item: (&'static str, i32),
) -> Result<(), Error> {
    if let Some(client) = connection.client_mut() {
        client.change(
            get_bank_key(item.0, client.this_player()),
            Value::from(item.1),
            vec![DataStorageOperation::Add(item.1 as f64)],
            true,
        )
    } else {
        Err(Error::ClientDisconnected)
    }

    // client
    //     .send(ClientMessage::Set(Set {
    //         key: get_bank_key(item.0),
    //         default: Value::from(1),
    //         want_reply: true,
    //         operations: vec![DataStorageOperation::Add(Value::from(item.1))],
    //     }))
    //     .await
}

/// Reset the banks contents to nothing. Used for resetting the values if needed.
// TODO Remove?
/*pub(crate) async fn _reset_bank(
    client: &mut ArchipelagoClient,
) -> Result<(), Box<dyn std::error::Error>> {
    get_bank().write()?.iter_mut().for_each(|(_k, v)| {
        *v = 0; // Set each bank item in the map to 0
    });
    for item in constants::get_items_by_category(ItemCategory::Consumable) {
        client
            .send(ClientMessage::Set(Set {
                key: get_bank_key(item),
                default: Value::from(0),
                want_reply: true,
                operations: vec![DataStorageOperation::Default],
            }))
            .await?;
    }
    Ok(())
}*/

pub(crate) fn read_values(
    client: &mut Client
) -> Result<(), Error> {
    let mut keys = vec![];
        for item in constants::get_items_by_category(ItemCategory::Consumable) {
            keys.push(get_bank_key(item, client.this_player()));
        }
        let rec = client.get(keys);
// TODO
        //client.get(ClientMessage::Get(Get { keys })).await?;
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
        create_hook!(
            USE_ITEM_ADDR,
            use_item,
            ORIGINAL_USE_ITEM,
            "Use item in Inv Screen"
        );
    }
    Ok(())
}

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
    let bank = get_bank().read().unwrap();
    for item in constants::get_items_by_category(ItemCategory::Consumable) {
        let item_id = ITEM_MAP.get_by_left(item).copied().unwrap();
        let item_address = param_1 + 0xC + item_id as usize;
        unsafe {
            replace_single_byte(
                item_address,
                read_data_from_address::<u8>(item_address) + (*bank.get(item).unwrap() as u8),
            );
        }
    }
}

pub const CLOSE_INV_SCREEN_ADDR: usize = 0x87460;
pub static ORIGINAL_CLOSE_INV_SCREEN: OnceLock<unsafe extern "C" fn(param_1: usize)> =
    OnceLock::new();

// Upon closing the game copies the contents of the status screen "inv" back to the real inventory
pub fn close_inv_screen(param_1: usize) {
    log::debug!("Closing Inv Screen");
    let bank = get_bank().read().unwrap();
    for item in constants::get_items_by_category(ItemCategory::Consumable) {
        let item_id = ITEM_MAP.get_by_left(item).copied().unwrap();
        let status_item_address = param_1 + 0xC + item_id as usize;
        let real_value = (read_data_from_address::<u8>(status_item_address) as i32
            - *bank.get(item).unwrap_or(&0))
        .max(0);

        unsafe {
            replace_single_byte(status_item_address, real_value as u8);
        }
    }

    unsafe {
        if let Some(orig) = ORIGINAL_CLOSE_INV_SCREEN.get() {
            orig(param_1)
        }
    }
}

pub const USE_ITEM_ADDR: usize = 0x2affc0;
pub static ORIGINAL_USE_ITEM: OnceLock<unsafe extern "C" fn(param_1: usize)> = OnceLock::new();
pub fn use_item(param_1: usize) {
    let item_index = read_data_from_address::<i32>(param_1 + 0x4148)
        + read_data_from_address::<i32>(param_1 + 0x411C);
    // This should be fine, I don't care about other items
    if (0..4).contains(&item_index) {
        let item_name = match item_index {
            0 => "Vital Star S",
            1 => "Vital Star L",
            2 => "Devil Star",
            3 => "Holy Water",
            _ => "",
        };
        log::debug!("Item selected: {}", item_name);
        let bank = get_bank().read().unwrap();
        if *bank.get(item_name).unwrap() > 0 {
            modify_bank_message(item_name, -1);
        }
    }
    unsafe {
        if let Some(orig) = ORIGINAL_USE_ITEM.get() {
            orig(param_1)
        }
    }
}
