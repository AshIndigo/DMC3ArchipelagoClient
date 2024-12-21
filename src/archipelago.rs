use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use anyhow::anyhow;
use std::collections::HashMap;
use std::fs::remove_file;
use std::io;
use std::io::BufRead;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Receiver;
use crate::{cache, hook};
use crate::cache::CustomGameData;
use crate::hook::Location;

// An ungodly mess
pub async fn connect_archipelago() -> Result<ArchipelagoClient, anyhow::Error> {
    let url = input("Archipelago URL: ")?;
    println!("url: {}", url);

    let mut client: Result<ArchipelagoClient, ArchipelagoError> =
        Err(ArchipelagoError::ConnectionClosed);
    if !cache::check_for_cache_file() { // If the cache file does not exist, then it needs to be acquired
        client = ArchipelagoClient::with_data_package(&url, Some(vec!["Devil May Cry 3".parse()?])).await;
        match &client {
            Ok(cl) => match &cl.data_package() { // Write the data package to a local cache file
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
    } else { // If the cache exists, then connect normally and verify the cache file
        client = ArchipelagoClient::new(&url).await;
        match client {
            Ok(ref mut cl) => {
                let option = cache::check_checksums(cl.room_info()).await;
                match option {
                    None => println!("Checksums check out!"),
                    Some(failures) => { // If there are checksums that don't match, obliterate the cache file and reconnect to obtain the data package
                        println!("Checksums check failures: {:?}", failures);
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
    println!("Connecting to url");
    match client {
        // Whether we have a client
        Ok(mut cl) => {
            println!("Attempting room connection");
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
                    println!("Connected info: {:?}", stat);
                    Ok(cl)
                }
                _err => Err(anyhow!("Failed to connect to room")),
            }
        }

        _ => Err(anyhow!("Failed to connect to server")),
    }
}

pub unsafe fn run_setup(cl: &ArchipelagoClient, data: CustomGameData) {
    println!("Running setup");
    hook::rewrite_mode_table();
    match cl.data_package() {
        Some(dat) => {
            println!("Data package exists: {:?}", dat);
            println!(
                "Item to ID: {:#?}",
                &dat.games["Devil May Cry 3"].item_name_to_id
            );
            println!(
                "Loc to ID: {:#?}",
                &dat.games["Devil May Cry 3"].location_name_to_id
            );
        }
        None => {
            println!("No data package found, using cached data");

        }
    }
}

pub async fn handle_things(cl: &mut ArchipelagoClient, rx: &Arc<Mutex<Receiver<Location>>>) {
    if let Ok(rec) = rx.lock() {
        while let Ok(item) = rec.try_recv() { // See if there's an item!
            println!("Processing item: {}", item); // TODO Need to handle offline storage... if the item cant be sent it needs to be buffered
            match cl.location_checks(vec![item.room]).await {
                Ok(_) => { println!("Location checks successful"); },
                Err(err) => { println!("Failed to check location: {}", err); }
            }
        }
    }
    println!("Ready for receiving");
        match cl.recv().await { // TODO I don't think I've seen this go off...
        Ok(opt_msg) => match opt_msg {
            None => {
                println!("Received None for msg");
            }
            Some(msg) => {
                println!("Received message: {:?}", msg);
            }
        },
        Err(err) => {
            println!("Failed to receive data: {}", err)
        }
    }
}

fn input(text: &str) -> Result<String, anyhow::Error> {
    println!("{}", text);

    Ok(io::stdin().lock().lines().next().unwrap()?)
}

// async fn disconnect_archipelago() {
//     match CLIENT {
//         Some(_client) => {
//             println!("Disconnecting from Archipelago server...");
//         }
//         None => {
//             println!("Not connected")
//         }
//     }
// }

// async fn perform_connection(url: String) -> Result<ArchipelagoClient, ArchipelagoError> {
//     let result = ArchipelagoClient::new(&url).await;
//     result
//     // match result {
//     //     Ok(res) => Some(res),
//     //     Err(err) => { println!("{}", err); None },
//     // }
// }