use crate::cache::{CustomGameData};
use crate::hook::{modify_itm_table, Location};
use crate::{cache, constants, hook, tables};
use anyhow::{anyhow};
use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{remove_file, File};
use std::io;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use once_cell::sync::OnceCell;
use crate::constants::get_locations;

pub static MAPPING: OnceCell<Mapping> = OnceCell::new();
pub static DATA_PACKAGE: OnceCell<CustomGameData> = OnceCell::new();

// An ungodly mess
pub async fn connect_archipelago() -> Result<ArchipelagoClient, anyhow::Error> {
    let url = input("Archipelago URL: ")?;
    log::info!("url: {}", url);

    let mut client: Result<ArchipelagoClient, ArchipelagoError> =
        Err(ArchipelagoError::ConnectionClosed);
    if !cache::check_for_cache_file() {
        // If the cache file does not exist, then it needs to be acquired
        client = ArchipelagoClient::with_data_package(&url, Some(vec!["Devil May Cry 3".parse()?]))
            .await;
        match &client {
            Ok(cl) => match &cl.data_package() {
                // Write the data package to a local cache file
                None => return Err(anyhow!("Data package does not exist")),
                Some(ref dp) => {
                    let mut clone_data = HashMap::new();
                    let _ = &dp.games.iter().for_each(|g| {
                        let dat = CustomGameData {
                            item_name_to_id: g.1.item_name_to_id.clone(),
                            location_name_to_id: g.1.location_name_to_id.clone(),
                        };
                        clone_data.insert(g.0.clone(), dat);
                    });
                    cache::write_cache(clone_data, cl.room_info())
                        .await
                        .expect("Failed to write cache file"); // TODO Probably shouldn't expect, and instead handle properly
                }
            },
            Err(er) => return Err(anyhow!("Failed to connect to (Data) Archipelago: {}", er)),
        }
    } else {
        // If the cache exists, then connect normally and verify the cache file
        client = ArchipelagoClient::new(&url).await;
        match client {
            Ok(ref mut cl) => {
                let option = cache::check_checksums(cl.room_info()).await;
                match option {
                    None => log::info!("Checksums check out!"),
                    Some(failures) => {
                        // If there are checksums that don't match, obliterate the cache file and reconnect to obtain the data package
                        log::info!("Checksums check failures: {:?}", failures);
                        remove_file("cache.json")?;
                        client = Err(ArchipelagoError::ConnectionClosed); // TODO Figure out a better way to do this
                        return Err(anyhow!("Reconnecting to grab cache!"));
                    }
                }
            }
            Err(er) => return Err(anyhow!("Failed to connect to Archipelago: {}", er)),
        }
    }
    let name = input("Name: ")?;
    let password = input("Password (Leave blank if unneeded): ")?;
    log::info!("Connecting to url");
    match client {
        // Whether we have a client
        Ok(mut cl) => {
            log::info!("Attempting room connection");
            let res = cl.connect(
                "Devil May Cry 3",
                &name,
                Some(&password),
                Option::from(0b101),
                vec!["AP".to_string()],
                true,
            );
            match res.await {
                Ok(stat) => {
                    log::info!("Connected info: {:?}", stat);
                    Ok(cl)
                }
                _err => Err(anyhow!("Failed to connect to room")),
            }
        }

        _ => Err(anyhow!("Failed to connect to server")),
    }
}

pub async unsafe fn run_setup(cl: &mut ArchipelagoClient) {
    log::info!("Running setup");
    hook::rewrite_mode_table();
    match cl.data_package() {
        Some(dat) => {
            log::info!("Data package exists: {:?}", dat);
            log::info!(
                "Item to ID: {:#?}",
                &dat.games["Devil May Cry 3"].item_name_to_id
            );
            log::info!(
                "Loc to ID: {:#?}",
                &dat.games["Devil May Cry 3"].location_name_to_id
            );
            match DATA_PACKAGE.set(CustomGameData {
                item_name_to_id: cl.data_package().unwrap().games.get("Devil May Cry 3").unwrap().item_name_to_id.clone(),
                location_name_to_id: cl.data_package().unwrap().games.get("Devil May Cry 3").unwrap().location_name_to_id.clone(),
            }) {
                Ok(_) => {}
                Err(_) => {}
            }
        }
        None => {
            log::info!("No data package found, using cached data");
            match DATA_PACKAGE.set(cache::read_cache().expect("Expected cache file...")) {
                Ok(_) => {}
                Err(_) => {}
            }
        }
    }
    match load_location_map() {
        None => {}
        Some(mappings) => {
            match MAPPING.set(mappings) {
                Ok(_) => {}
                Err(_) => {log::info!("Failed to set cell!");}
            }
        }
    }
    use_mappings();
}



#[derive(Deserialize, Serialize)]
pub struct Mapping {
    // For mapping JSON
    pub seed: String,
    pub slot: String,
    pub items: HashMap<String, String>,
    pub starter_items: Vec<String>,
}

pub struct ItemEntry {
    // Represents an item on the ground
    pub offset: usize,    // Offset for the item table
    pub room_number: u16, // Room number
    pub item_id: u8,      // Default Item ID
    pub mission: u8,      // Mission Number
    pub adjudicator: bool // Adjudicator
                          // TODO Secret
}

fn use_mappings() {
    match MAPPING.get() {
        Some(data) => {
            for (k, v) in data.items.iter() {
                match get_locations().get(k as &str) {
                    Some(entry) => match tables::get_item_id(v) {
                        Some(id) => unsafe { modify_itm_table(entry.offset, id) },
                        None => {
                            log::warn!("Item not found: {}", v);
                        }
                    },
                    None => {
                        log::warn!("Location not found: {}", k);
                    }
                }
            }
        }
        None => {log::warn!("No mapping found");}
    }
}

pub fn load_location_map() -> Option<Mapping> {
    match Path::new("mappings.json").try_exists() {
        Ok(res) => {
            if res == true {
                log::info!("Mapping file Exists!");
                let file = File::open("mappings.json");
                match file {
                    Ok(mapping) => {
                        let reader = BufReader::new(mapping);
                        let mut json_reader = serde_json::Deserializer::from_reader(reader);
                        let json = Mapping::deserialize(&mut json_reader);
                        match json {
                            Ok(map) => {
                                log::info!("Mapping location mapped successfully!");
                                Some(map)
                            }
                            Err(_) => None,
                        }
                    }
                    Err(err) => {
                        log::info!("Mapping file doesn't exist?: {:?}", err);
                        None
                    }
                }
            } else {
                log::info!("Mapping file doesn't exist");
                None
            }
        }
        Err(_) => {
            log::info!("Failed to check for cache file!");
            None
        }
    }
}

pub async fn handle_things(cl: &mut ArchipelagoClient, rx: &Arc<Mutex<Receiver<Location>>>) {
    if let Ok(rec) = rx.lock() {
        while let Ok(item) = rec.try_recv() {
            // See if there's an item!
            log::info!("Processing item: {}", item); // TODO Need to handle offline storage... if the item cant be sent it needs to be buffered
            match MAPPING.get() {
                Some(mapping_data) => {
                    /*
                    Need to somehow map the data I have back to the right room
                    I cant do just by item id+room+mission because room's may have multiple items that are the same.
                    I.e. M3 R5 has two items in it, that are the same in vanilla (0x09)
                    If I could somehow get a hold of the offset for the item table, that is definitely unique and I could use that
                    Event given items are going to be their own beast though... no offset table for those

                    Also, need to filter out items I don't care about (Can't be sending red orbs)
                    */
                    for (k, v) in get_locations() {
                        //log::debug!("Checking room {} vs {} and mission {} vs {}", v.room_number as i32, item.room, v.mission as i32, item.mission);
                        if v.room_number as i32 == item.room { // && v.mission as i32 == item.mission { // First confirm the room and mission number
                            //log::debug!("Room and mission check out!");
                            log::debug!("Checking location items: 0x{:x} vs 0x{:x}", tables::get_item_id(mapping_data.items.get(k).unwrap()).unwrap(), item.item_id as u8);
                            if tables::get_item_id(mapping_data.items.get(k).unwrap()).unwrap() == item.item_id as u8 { // Then see if the item picked up matches the specified in the map
                                match DATA_PACKAGE.get() {
                                    None => log::error!("Data Package not found!"),
                                    Some(dp) => {
                                       match dp.location_name_to_id.get(k) {
                                           None => log::error!("Location not found: {}", k),
                                           Some(loc_id) => {
                                               match cl.location_checks(vec![loc_id.clone()]).await {
                                                   Ok(_) => {
                                                       log::info!("Location check successful: {} ({})", k, loc_id);
                                                   }
                                                   Err(err) => {
                                                       log::info!("Failed to check location: {}", err);
                                                   }
                                               }
                                           }
                                       }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    match cl.recv().await {
        Ok(opt_msg) => match opt_msg {
            None => {
            }
            Some(msg) => {
                log::info!("Received message: {:?}", msg); // TODO Actually handle the messages and make them look nice?
            }
        },
        Err(ArchipelagoError::NetworkError(err)) => {
            log::info!("Failed to receive data, reconnecting: {}", err);
            match connect_archipelago().await {
                Ok(client) => {
                    *cl = client;
                }
                Err(_) => {}
            }
        }
        Err(err) => {
            log::info!("Failed to receive data: {}", err)
        }
    }
}

fn input(text: &str) -> Result<String, anyhow::Error> {
    log::info!("{}", text);

    Ok(io::stdin().lock().lines().next().unwrap()?)
}

// async fn disconnect_archipelago() {
//     match CLIENT {
//         Some(_client) => {
//             log::info!("Disconnecting from Archipelago server...");
//         }
//         None => {
//             log::info!("Not connected")
//         }
//     }
// }

// async fn perform_connection(url: String) -> Result<ArchipelagoClient, ArchipelagoError> {
//     let result = ArchipelagoClient::new(&url).await;
//     result
//     // match result {
//     //     Ok(res) => Some(res),
//     //     Err(err) => { log::info!("{}", err); None },
//     // }
// }
