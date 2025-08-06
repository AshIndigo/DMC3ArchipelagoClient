use crate::constants::ItemEntry;
use crate::constants::*;
use crate::data::generated_locations;
use crate::mapping::Mapping;
use crate::ui::ui::{CHECKLIST, CONNECTION_STATUS};
use crate::utilities::{
    DMC3_ADDRESS, get_mission, get_room, read_data_from_address, replace_single_byte,
};
use crate::{archipelago, check_handler, create_hook, item_sync, mapping, save_handler, utilities};
use archipelago_rs::client::ArchipelagoClient;
use archipelago_rs::protocol::{Bounce, ClientMessage};
use log::error;
use minhook::{MH_STATUS, MinHook};
use std::arch::asm;
use std::collections::HashMap;
use std::ffi::c_longlong;
use std::ptr::read_unaligned;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, OnceLock, RwLockReadGuard};
use std::{ptr, slice, thread};
use serde_json::json;
use tokio::sync::Mutex;
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::PAGE_EXECUTE_READWRITE;

pub(crate) const DUMMY_ID: u8 = 0x20;
static HOOKS_CREATED: AtomicBool = AtomicBool::new(false);

pub(crate) fn install_initial_functions() {
    if !HOOKS_CREATED.load(Ordering::SeqCst) {
        unsafe {
            match create_hooks() {
                Ok(_) => {
                    HOOKS_CREATED.store(true, Ordering::SeqCst);
                }
                Err(err) => {
                    log::error!("Failed to create hook: {:?}", err);
                }
            }
        }
    }
    enable_hooks();
}

// 23d680 - Pause menu event? Hook in here to do rendering
unsafe fn create_hooks() -> Result<(), MH_STATUS> {
    unsafe {
        create_hook!(
            ITEM_HANDLE_PICKUP_ADDR,
            check_handler::item_non_event,
            ORIGINAL_HANDLE_PICKUP,
            "Non event item"
        );
        create_hook!(
            ITEM_PICKED_UP_ADDR,
            check_handler::item_event,
            ORIGINAL_ITEM_PICKED_UP,
            "Event item"
        );
        create_hook!(
            RESULT_CALC_ADDR,
            check_handler::mission_complete_check,
            ORIGINAL_RESULT_CALC,
            "Mission complete"
        );
        create_hook!(
            ITEM_SPAWNS_ADDR,
            item_spawns_hook,
            ORIGINAL_ITEM_SPAWNS,
            "Item Spawn"
        );
        create_hook!(
            EDIT_EVENT_HOOK_ADDR,
            edit_event_drop,
            ORIGINAL_EDIT_EVENT,
            "Event table"
        );
        create_hook!(
            0x23a7b0usize,
            setup_inventory_for_mission,
            ORIGINAL_MISSION_INV,
            "Setup mission inventory"
        );
        create_hook!(
            EQUIPMENT_SCREEN_ADDR,
            edit_initial_index,
            ORIGINAL_EQUIPMENT_SCREEN,
            "Change equipment screen index"
        );
        create_hook!(
            DAMAGE_CALC_ADDR,
            monitor_hp,
            ORIGINAL_DAMAGE_CALC,
            "HP Monitor"
        );
        save_handler::setup_save_hooks()?;
        // create_hook!(
        //     RENDER_TEXT_ADDR,
        //     parry_text,
        //     ORIGINAL_RENDER_TEXT,
        //     "Render Text"
        // );
    }
    Ok(())
}

fn enable_hooks() {
    let addresses: Vec<usize> = vec![
        ITEM_HANDLE_PICKUP_ADDR,
        ITEM_PICKED_UP_ADDR,
        RESULT_CALC_ADDR,
        ITEM_SPAWNS_ADDR,
        EDIT_EVENT_HOOK_ADDR,
        0x23a7b0usize,
        // Save handler
        save_handler::LOAD_GAME_ADDR,
        save_handler::SAVE_GAME_ADDR,
        EQUIPMENT_SCREEN_ADDR,
        DAMAGE_CALC_ADDR,
    ];
    addresses.iter().for_each(|addr| unsafe {
        match MinHook::enable_hook((*DMC3_ADDRESS.read().unwrap() + addr) as *mut _) {
            Ok(_) => {}
            Err(err) => {
                log::error!("Failed to enable {:#X} hook: {:?}", addr, err);
            }
        }
    })
}

pub(crate) static CLIENT: LazyLock<Mutex<Option<ArchipelagoClient>>> =
    LazyLock::new(|| Mutex::new(None));

pub fn edit_event_drop(param_1: i64, param_2: i32, param_3: i64) {
    let (mapping, mission_event_tables, checked_locations) =
        if let (Ok(mapping), Some(mission_event_tables), Some(checked_locations)) = (
            mapping::MAPPING.read(),
            EVENT_TABLES.get(&get_mission()),
            archipelago::get_checked_locations().lock().ok(),
        ) && mapping.is_some()
        {
            (mapping, mission_event_tables, checked_locations)
        } else {
            unsafe {
                if let Some(original) = ORIGINAL_EDIT_EVENT.get() {
                    original(param_1, param_2, param_3);
                }
            }
            return;
        };
    match mapping.as_ref() {
        None => log::debug!("How did we get here?"),
        Some(mapping) => {
            unsafe {
                // For each table
                for event_table in mission_event_tables {
                    for event in &event_table.events {
                        if checked_locations.contains(&event_table.location) {
                            log::debug!("Event loc checked: {}", &event_table.location);
                            match event.event_type {
                                // If the location has already been checked use DUMMY_ID as a dummy item.
                                EventCode::GIVE => {
                                    replace_single_byte(EVENT_TABLE_ADDR + event.offset, DUMMY_ID)
                                }
                                EventCode::CHECK => {
                                    replace_single_byte(EVENT_TABLE_ADDR + event.offset, DUMMY_ID)
                                }
                                EventCode::END => {
                                    replace_single_byte(EVENT_TABLE_ADDR + event.offset, DUMMY_ID)
                                }
                            }
                        } else {
                            log::debug!("Event loc not checked: {}", &event_table.location);
                            match event.event_type {
                                // Location has not been checked off! TODO Make the "check" event, a dummied out item
                                EventCode::GIVE => replace_single_byte(
                                    EVENT_TABLE_ADDR + event.offset,
                                    get_item_id(
                                        &*mapping
                                            .items
                                            .get(event_table.location)
                                            .unwrap()
                                            .item_name,
                                    )
                                    .unwrap(),
                                ),
                                EventCode::CHECK => {
                                    replace_single_byte(EVENT_TABLE_ADDR + event.offset, DUMMY_ID)
                                }
                                EventCode::END => {
                                    replace_single_byte(EVENT_TABLE_ADDR + event.offset, DUMMY_ID)
                                }
                            }
                        }
                    }
                }
                if let Some(original) = ORIGINAL_EDIT_EVENT.get() {
                    original(param_1, param_2, param_3);
                }
            }
        }
    }
}

/// Modify the game's code so the "pickup mode" table is correct
// start at 1B3944 -> 1B395A
// Set these from 02 to 01
pub(crate) fn rewrite_mode_table() {
    let table_address = ITEM_MODE_TABLE + *DMC3_ADDRESS.read().unwrap();
    let mut old_protect = 0;
    let length = 16;
    unsafe {
        VirtualProtect(
            table_address as *mut _,
            length, // Length of table I need to modify
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        let table = slice::from_raw_parts_mut(table_address as *mut u8, length);
        table.fill(0x01u8); // 0 = orbs, 1 = items, 2 = bad

        VirtualProtect(
            table_address as *mut _,
            length,
            old_protect,
            &mut old_protect,
        );
    }
}

/// Modifies Adjudicator Drops
pub(crate) unsafe fn modify_adjudicator_drop(mapping: &Mapping) {
    for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
        // Run through all locations
        if entry.adjudicator && entry.mission == get_mission() {
            // If Location is adjudicator and mission numbers match
            let item_id =
                get_item_id(&*mapping.items.get(*location_name).unwrap().item_name).unwrap(); // Get the item ID and replace
            unsafe {
                utilities::replace_single_byte_with_base_addr(ADJUDICATOR_ITEM_ID_1, item_id);
                utilities::replace_single_byte_with_base_addr(ADJUDICATOR_ITEM_ID_2, item_id);
            }
        }
    }
}

fn item_spawns_hook(unknown: i64) {
    unsafe {
        #[allow(unused_assignments)]
        let mut item_addr: *mut i32 = ptr::null_mut();
        let item_count: u32;
        asm!(
            "mov eax, [rcx+0x06]",
            out("eax") item_count, // Count of items in room
            clobber_abi("win64")
        );
        asm!(
            "lea eax, [rcx+0x10]",
            out("eax") item_addr, // Item address, needs to be [eax]'d
            clobber_abi("win64")
        );
        log::debug!("Item count: {} ({:#X})", item_count, item_count);
        let room_num: u16 = get_room() as u16;
        log::debug!("Room num: {} ({:#X})", room_num, room_num);
        //set_relevant_key_items();
        match mapping::MAPPING.read() {
            Ok(mapping) => match mapping.as_ref() {
                None => {
                    log::warn!("Mapping's are not set up. Logging debug info:");
                    for _i in 0..item_count {
                        let item_ref: &u32 = &*(item_addr as *const u32);
                        log::debug!(
                            "Item ID: {} ({:#X})",
                            get_item_name(*item_ref as u8),
                            *item_ref
                        );
                        item_addr = item_addr.byte_offset(0x14);
                    }
                }
                Some(mapping) => {
                    modify_adjudicator_drop(mapping);
                    modify_secret_mission_item(mapping);
                    for _i in 0..item_count {
                        let item_ref: &u32 = &*(item_addr as *const u32);
                        const EXTRA_OUTPUT: bool = false;
                        if EXTRA_OUTPUT {
                            log::debug!(
                                "Item ID: {} ({:#X})",
                                get_item_name(*item_ref as u8),
                                *item_ref
                            );
                        }
                        for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
                            check_and_replace_item(
                                location_name,
                                entry,
                                room_num,
                                &*mapping,
                                item_ref,
                                item_addr,
                            );
                        }
                        item_addr = item_addr.byte_offset(0x14);
                    }
                }
            },
            Err(e) => {
                log::error!("Failed to get mappings lock: {}", e);
            }
        }
        if let Some(original) = ORIGINAL_ITEM_SPAWNS.get() {
            original(unknown);
        }
    }
}

fn modify_secret_mission_item(mapping: &Mapping) {
    unsafe {
        for (location_name, _entry) in generated_locations::ITEM_MISSION_MAP
            .iter()
            .filter(|(_location_name, entry)| entry.room_number as i32 == get_room())
        {
            log::debug!(
                "Replaced secret mission with: {:#X}",
                get_item_id(&*mapping.items.get(*location_name).unwrap().item_name).unwrap()
            );
            // Get the item ID and replace
            utilities::replace_single_byte_with_base_addr(
                SECRET_MISSION_ITEM,
                *ITEM_ID_MAP.get("Remote").unwrap(),
            );
        }
    }
}

fn monitor_hp(damage_calc: usize, param_1: usize, param_2: usize, param_3: usize) {
    unsafe { ORIGINAL_DAMAGE_CALC.get().unwrap()(damage_calc, param_1, param_2, param_3) }
    let char_data_ptr: usize =
        read_data_from_address(*DMC3_ADDRESS.read().unwrap() + utilities::ACTIVE_CHAR_DATA);
    unsafe {
        let hp = read_unaligned((char_data_ptr + 0x411C) as *mut f32);
        if hp == 0.0 {
            log::debug!("Dante died!");
            match mapping::MAPPING.read() {
                Ok(mapping) => match mapping.as_ref() {
                    None => {}
                    Some(mapping) => {
                        if mapping.death_link {
                            thread::spawn(async move || {
                                if let Some(ref mut client) = CLIENT.lock().await.as_mut() {
                                    client
                                        .send(ClientMessage::Bounce(Bounce {
                                            games: None,
                                            slots: None,
                                            tags: Some(vec!["DeathLink".to_string()]),
                                            data: json!({ // TODO Filling in data here
                                                "time": "0.0",
                                                "source": "Ash",
                                                "cause": "skill issue"
                                            }),
                                        }))
                                        .await
                                        .expect("Failed to send DeathLink message");
                                }
                            });
                        }
                    }
                },
                Err(err) => {
                    error!("Failed to get mapping lock: {}", err);
                }
            }
        }
    }
}

// TODO Consolidate this down?
fn edit_initial_index(custom_weapon: usize) -> i32 {
    let current_inv_addr = utilities::get_inv_address();
    if current_inv_addr.is_none() {
        log::error!(
            "Failed to get inventory address in {}",
            "edit_initial_index"
        );
        unsafe {
            if let Some(original) = ORIGINAL_EQUIPMENT_SCREEN.get() {
                return original(custom_weapon);
            }
        }
    }
    let base = *DMC3_ADDRESS.read().unwrap();
    let mut starting_index = 0;
    if read_data_from_address::<u8>(custom_weapon + 0x419E) == 4 {
        // Gun
        for index in 0..5 {
            if read_data_from_address::<bool>(current_inv_addr.unwrap() + 0x58 + index) {
                starting_index = index;
                break;
            }
        }
        if read_data_from_address::<bool>(current_inv_addr.unwrap() + 0x5D) {
            // Kalina Ann
            starting_index = 4;
        }
    } else {
        // Melee
        for index in 0..3 {
            if read_data_from_address::<bool>(current_inv_addr.unwrap() + 0x52 + index) {
                starting_index = index;
                break;
            }
        }
        if read_data_from_address::<bool>(current_inv_addr.unwrap() + 0x56) {
            // Nevan
            starting_index = 3;
        }
        if read_data_from_address::<bool>(current_inv_addr.unwrap() + 0x57) {
            // Beowulf
            starting_index = 4;
        }
    }
    unsafe {
        replace_single_byte(base + 0x28CC36, starting_index as u8);
    }
    unsafe { ORIGINAL_EQUIPMENT_SCREEN.get().unwrap()(custom_weapon) }
}

unsafe fn check_and_replace_item(
    location_name: &&str,
    entry: &ItemEntry,
    room_num: u16,
    mapping: &Mapping,
    item_ref: &u32,
    item_addr: *mut i32,
) {
    unsafe {
        // Skipping if location file has room as 0, that means its either event or not done
        if entry.room_number == 0 {
            return;
        }
        //log::debug!("Room number X: {} Room number memory: {}, Item ID X: 0x{:x}, Item ID Memory: 0x{:x}", entry.room_number, room_num, entry.item_id, *item_ref);
        if entry.room_number == room_num && entry.item_id as u32 == *item_ref && !entry.adjudicator
        {
            if !dummy_replace(location_name, item_addr, entry.offset) {
                let ins_val = get_item_id(&*mapping.items.get(*location_name).unwrap().item_name); // Scary

                if ITEM_MAP.get(&entry.item_id).unwrap().category == ItemCategory::Key {
                    *item_addr = *(ITEM_ID_MAP.get("Remote").unwrap()) as i32; // 0x26 is high roller card, what is used for remote items.
                } else {
                    *item_addr = ins_val.unwrap() as i32;
                }
                log::info!(
                    "Replaced item in room {} ({}) with {} {:#X}",
                    entry.room_number,
                    location_name,
                    get_item_name(*item_ref as u8),
                    *item_addr
                );
            }
        }
    }
}

/// Replaces an item with a dummy one in order to not immediately proc end events upon entering the location's room
fn dummy_replace(location_key: &&str, item_addr: *mut i32, _offset: usize) -> bool {
    // Get event tables for mission and then each END event
    if let Some(event_tables) = EVENT_TABLES.get(&get_mission()) {
        for event_table in event_tables
            .iter()
            .filter(|table| table.location == *location_key)
        {
            for _event in event_table
                .events
                .iter()
                .filter(|event| event.event_type == EventCode::END)
            {
                // Then if location in question is checked, replace the item with a dummy and return true
                if let Ok(checked_locations) = archipelago::get_checked_locations().lock() {
                    if checked_locations.contains(location_key) {
                        unsafe {
                            *item_addr = DUMMY_ID as i32;
                        }
                        log::info!("Replaced item at {} with dummy item", location_key);
                        return true;
                    }
                } else {
                    log::error!("Failed to lock checked locations vec");
                }
            }
        }
    }
    false
}

fn set_relevant_key_items() {
    if CONNECTION_STATUS.load(Ordering::Relaxed) != 1 {
        return;
    }
    let checklist: RwLockReadGuard<HashMap<String, bool>> =
        CHECKLIST.get().unwrap().read().unwrap();
    let current_inv_addr = utilities::get_inv_address();
    if current_inv_addr.is_none() {
        return;
    }
    log::debug!("Current INV Addr: {:#X}", current_inv_addr.unwrap());
    let mut flag: u8;
    log::debug!("Current mission: {}", get_mission());
    match MISSION_ITEM_MAP.get(&(get_mission())) {
        None => {} // No items for the mission
        Some(item_list) => {
            for item in item_list.into_iter() {
                if *checklist.get(*item).unwrap_or(&false) {
                    flag = 0x01;
                    log::debug!("Item Relevant to mission {}", *item)
                } else {
                    flag = 0x00;
                }
                let item_addr =
                    current_inv_addr.unwrap() + ITEM_OFFSET_MAP.get(item).unwrap().clone() as usize;
                log::debug!(
                    "Attempting to replace at address: {:#X} with flag {:#X}",
                    item_addr,
                    flag
                );
                unsafe { replace_single_byte(item_addr, flag) };
            }
        }
    }
    // Setting weapons
    // TODO Probably need to modify equipped values as well
    for weapon in get_items_by_category(ItemCategory::Weapon) {
        if *checklist.get(weapon).unwrap_or(&false) {
            flag = 0x01;
            log::debug!("Adding weapon/style to inventory {}", weapon);
            if weapon == "Cerberus" && read_data_from_address::<u8>(0x045FF2D8 + 0x01) == 0xFF {
                unsafe { replace_single_byte(0x045FF2D8 + 0x01, 0x01) };
            }
            if weapon == "Shotgun" && read_data_from_address::<u8>(0x045FF2D8 + 0x03) == 0xFF {
                unsafe { replace_single_byte(0x045FF2D8 + 0x03, 0x06) };
            }
        } else {
            flag = 0x00;
        }
        let item_addr =
            current_inv_addr.unwrap() + ITEM_OFFSET_MAP.get(weapon).unwrap().clone() as usize;
        log::trace!(
            "Attempting to replace at address: {:#X} with flag {:#X}",
            item_addr,
            flag
        );
        unsafe { replace_single_byte(item_addr, flag) };
    }
    //item_sync::validate_equipment(&checklist);
}

/// Disable hooks, used for disconnecting
pub fn disable_hooks() -> Result<(), MH_STATUS> {
    let base_address = *DMC3_ADDRESS.read().unwrap();
    unsafe {
        MinHook::disable_hook((base_address + ITEM_HANDLE_PICKUP_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + ITEM_PICKED_UP_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + RESULT_CALC_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + ITEM_SPAWNS_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + EDIT_EVENT_HOOK_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + 0x23a7b0usize) as *mut _)?;
        MinHook::disable_hook((base_address + save_handler::LOAD_GAME_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + save_handler::SAVE_GAME_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + EQUIPMENT_SCREEN_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + DAMAGE_CALC_ADDR) as *mut _)?;
        Ok(())
    }
}

pub static ORIGINAL_MISSION_INV: OnceLock<unsafe extern "C" fn(param_1: c_longlong) -> bool> =
    OnceLock::new();

pub fn setup_inventory_for_mission(param_1: c_longlong) -> bool {
    let mut res = false;
    unsafe {
        if let Some(original) = ORIGINAL_MISSION_INV.get() {
            res = original(param_1);
        }
    }
    set_relevant_key_items();
    utilities::set_max_hp(f32::min(
        (6.0 * ONE_ORB) + item_sync::BLUE_ORBS_OBTAINED.load(Ordering::SeqCst) as f32 * ONE_ORB,
        MAX_HP,
    ));
    utilities::set_max_magic(f32::min(
        item_sync::BLUE_ORBS_OBTAINED.load(Ordering::SeqCst) as f32 * ONE_ORB,
        MAX_MAGIC,
    ));
    res
}

/// The mapping data at dmc3.exe+5c4c20+1A00
/// This table dictates what item is in what room. Only relevant for consumables and blue orb fragments
pub fn modify_item_table(offset: usize, id: u8) {
    unsafe {
        // let start_addr = 0x5C4C20usize; dmc3.exe+5c4c20+1A00
        // let end_addr = 0x5C4C20 + 0xC8; // 0x5C4CE8
        let true_offset = offset + *DMC3_ADDRESS.read().unwrap() + 0x1A00usize; // MFW I can't do my offsets correctly
        if offset == 0x0 {
            return; // Undecided/ignorable
        }
        let mut old_protect = 0;
        VirtualProtect(
            true_offset as *mut _,
            4, // Length of table I need to modify
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        let table = slice::from_raw_parts_mut(true_offset as *mut u8, 4);

        table[3] = id;

        VirtualProtect(true_offset as *mut _, 4, old_protect, &mut old_protect);
        // log::trace!(
        //     "Modified Item Table: Address: 0x{:x}, ID: 0x{:x}, Offset: 0x{:x}",
        //     true_offset,
        //     id,
        //     offset
        // ); // Shushing this for now
    }
}

/// Restore the mode table to its original values
pub(crate) fn restore_item_table() {
    generated_locations::ITEM_MISSION_MAP
        .iter()
        .filter(|item| item.1.offset != 0x0) // item.1 is the entry
        .for_each(|(_key, val)| {
            unsafe {
                let true_offset = val.offset + *DMC3_ADDRESS.read().unwrap() + 0x1A00usize;
                let mut old_protect = 0;
                VirtualProtect(
                    true_offset as *mut _,
                    4, // Length of table I need to modify
                    PAGE_EXECUTE_READWRITE,
                    &mut old_protect,
                );

                let table = slice::from_raw_parts_mut(true_offset as *mut u8, 4);

                table[3] = val.item_id;

                VirtualProtect(true_offset as *mut _, 4, old_protect, &mut old_protect);
            }
        })
}

/// Set the modified modes back to 1 from 2
pub(crate) fn restore_mode_table() {
    let table_address = ITEM_MODE_TABLE + *DMC3_ADDRESS.read().unwrap();
    let mut old_protect = 0;
    let length = 16;
    unsafe {
        VirtualProtect(
            table_address as *mut _,
            length, // Length of table I need to modify
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        let table = slice::from_raw_parts_mut(table_address as *mut u8, length);
        table.fill(0x02u8); // 0 = orbs, 1 = items, 2 = bad

        VirtualProtect(
            table_address as *mut _,
            length,
            old_protect,
            &mut old_protect,
        );
    }
}
