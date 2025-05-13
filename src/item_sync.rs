use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;
use archipelago_rs::protocol::{DataPackageObject, ReceivedItems};
use serde::{Deserialize, Serialize};

const SAVE_FILE: &str = "archipelago.json";

#[derive(Deserialize, Serialize, Debug)]
pub struct SaveData {
    save_indices: HashMap<String, i32>
}

pub async fn write_save_data(data: SaveData) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(SAVE_FILE)?;
    file.write_all(serde_json::to_string_pretty(&data)?.as_bytes())?;
    file.flush()?;
    Ok(())
}

pub(crate) fn read_save_data() -> Result<DataPackageObject, anyhow::Error> {
    let cache = DataPackageObject::deserialize(&mut serde_json::Deserializer::from_reader(
        BufReader::new(File::open(SAVE_FILE)?),
    ))?;
    Ok(cache)
}

pub fn check_for_save_data() -> bool {
    Path::new(SAVE_FILE).try_exists().unwrap_or_else(|err| {
        log::info!("Failed to check for cache file: {}", err);
        false
    })
}


pub(crate) fn handle_received_items_packet(items: ReceivedItems) -> Result<(), Box<dyn Error>>{
    // READ https://github.com/ArchipelagoMW/Archipelago/blob/main/docs/network%20protocol.md#synchronizing-items
    for item in items.items.iter() {
        /*    unsafe { // TODO This will crash due to a bug with display_message (Needs another "vanilla" message to prep)
            utilities::display_message(format!(
                "Received {}!",
                constants::get_item(item.item as u64)
            ));
        }*/
        if item.item < 0x14 { // TODO Bank stuff broken
            // Consumables/orbs TODO
            //bank::add_item(client, item).await;
        }
    }
    log::debug!("Received items: {:?}", items.items);
    Ok(())
}