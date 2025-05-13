use crate::archipelago::{BANK, SLOT_NUMBER, TEAM_NUMBER};
use crate::constants::CONSUMABLES;
use crate::{constants, hook};
use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use archipelago_rs::protocol::{DataStorageOperation, NetworkItem};
use serde_json::Value;
use std::collections::HashMap;
use std::ops::SubAssign;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

pub(crate) async fn add_item(client: &mut ArchipelagoClient, item: &NetworkItem) {
    client
        .set(
            get_bank_key(&constants::get_item(item.item as u64).parse().unwrap()),
            Value::from(1),
            false,
            vec![DataStorageOperation::Add(Value::from(1))],
        )
        .await
        .unwrap();
}

pub static TX_BANK: OnceLock<Sender<String>> = OnceLock::new();

pub fn get_bank() -> &'static Mutex<HashMap<&'static str, i32>> {
    BANK.get_or_init(|| {
        Mutex::new(HashMap::from([
            (CONSUMABLES[0], 0),
            (CONSUMABLES[1], 0),
            (CONSUMABLES[2], 0),
            (CONSUMABLES[3], 0),
            (CONSUMABLES[4], 0),
        ]))
    })
}

pub fn setup_bank_channel() -> Receiver<String> {
    //Arc<Mutex<Receiver<String>>> {
    let (tx, rx) = mpsc::channel(64);
    TX_BANK.set(tx).expect("TX already initialized");
    rx
    //Arc::new(Mutex::new(rx))
}

pub(crate) fn get_bank_key(item: &String) -> String {
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
    //if let Some(item) = bank_rx.recv().await {
    match client.get(vec![get_bank_key(&item)]).await {
        Ok(val) => {
            let item_count = val
                .keys
                .get(&get_bank_key(&item))
                .unwrap()
                .as_i64()
                .unwrap();
            log::info!("{} is {}", item, item_count);
            if item_count > 0 {
                match client
                    .set(
                        get_bank_key(&item),
                        Value::from(1),
                        false,
                        vec![DataStorageOperation::Add(Value::from(-1))],
                    )
                    .await
                {
                    Ok(_) => {
                        log::debug!("{} subtracted", item);
                        hook::add_item(&item);
                        get_bank()
                            .lock()
                            .unwrap()
                            .get_mut(&*item.clone())
                            .unwrap()
                            .sub_assign(1);
                    }
                    Err(err) => {
                        log::error!("Failed to subtract item: {}", err);
                    }
                }
            }
        }
        Err(err) => {
            log::error!("Failed to get banker item: {}", err);
        }
    }
    //}
    Ok(()) // TODO
}
