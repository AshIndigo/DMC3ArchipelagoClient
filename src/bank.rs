use archipelago_rs::protocol::{DataStorageOperation, NetworkItem};
use serde_json::Value;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::atomic::Ordering;
use std::collections::HashMap;
use std::ops::SubAssign;
use archipelago_rs::client::ArchipelagoClient;
use crate::archipelago::{BANK, SLOT_NUMBER, TEAM_NUMBER};
use crate::{constants, hook};
use crate::constants::CONSUMABLES;

pub(crate) async fn add_item(client: &mut ArchipelagoClient, item: &NetworkItem) {
    client.set(get_bank_key(&constants::get_item(item.item as u64).parse().unwrap(), ), Value::from(1), false, vec![DataStorageOperation {
            replace: "add".to_string(),
            value: Value::from(1),
        }], )
        .await.unwrap();
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

pub fn setup_bank_channel() -> Arc<Mutex<Receiver<String>>> {
    let (tx, rx) = mpsc::channel();
    TX_BANK.set(tx).expect("TX already initialized");
    Arc::new(Mutex::new(rx))
}

pub(crate) fn get_bank_key(item: &String) -> String {
    format!(
        "team{}_slot{}_{}",
        TEAM_NUMBER.load(Ordering::SeqCst),
        SLOT_NUMBER.load(Ordering::SeqCst),
        item
    )
}

pub(crate) async fn handle_bank(mut client: ArchipelagoClient, bank_rx: &Arc<Mutex<Receiver<String>>>) {
    if let Ok(bank_rec) = bank_rx.lock() { // TODO Bank stuff issue
        while let Ok(item) = bank_rec.try_recv() {
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
                                vec![DataStorageOperation {
                                    replace: "add".to_string(),
                                    value: Value::from(-1),
                                }],
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
        }
    }
}