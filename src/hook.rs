use crate::archipelago::ArchipelagoData;
use crate::archipelago::{SLOT_NUMBER, TEAM_NUMBER, connect_archipelago};
use crate::bank::{setup_bank_add_channel, setup_bank_to_inv_channel};
use crate::constants::ItemEntry;
use crate::constants::*;
use crate::mapping::{Mapping, get_mappings};
use crate::ui::ui;
use crate::ui::ui::CHECKLIST;
use crate::utilities::{get_mission, get_room};
use crate::{archipelago, check_handler, generated_locations, utilities};
use anyhow::{Error, anyhow};
use archipelago_rs::client::ArchipelagoClient;
use archipelago_rs::protocol::ClientStatus;
use minhook::{MH_STATUS, MinHook};
use serde::Deserialize;
use std::arch::asm;
use std::collections::HashMap;
use std::convert::Into;
use std::ffi::c_longlong;
use std::io::BufReader;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{LazyLock, OnceLock, RwLock, RwLockWriteGuard};
use std::{fs, path, ptr, slice};
use tokio::sync::Mutex;
use winapi::um::libloaderapi::{FreeLibrary, GetModuleHandleW};
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::PAGE_EXECUTE_READWRITE;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    AllocConsole, ENABLE_VIRTUAL_TERMINAL_PROCESSING, FreeConsole, GetConsoleMode, GetStdHandle,
    STD_OUTPUT_HANDLE, SetConsoleMode,
};

pub fn create_console() {
    unsafe {
        if AllocConsole().is_ok() {
            pub fn enable_ansi_support() -> Result<(), Error> {
                // So we can have sweet sweet color
                unsafe {
                    let handle = GetStdHandle(STD_OUTPUT_HANDLE)?;
                    if handle == HANDLE::default() {
                        return Err(anyhow!(windows::core::Error::from_win32()));
                    }

                    let mut mode = std::mem::zeroed();
                    GetConsoleMode(handle, &mut mode)?;
                    SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING)?;
                    Ok(())
                }
            }
            match enable_ansi_support() {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Failed to enable ANSI support: {}", err);
                }
            }
            log::info!("Console created successfully!");
        } else {
            log::info!("Failed to allocate console!");
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn free_self() -> bool {
    unsafe {
        FreeConsole().expect("Unable to free console");
        let module_handle = GetModuleHandleW(ptr::null());
        if module_handle.is_null() {
            return false;
        }
        FreeLibrary(module_handle) != 0
    }
}

pub(crate) fn install_initial_functions() {
    setup_hooks().unwrap_or_else(|status| {
        // TODO Improve later, will setup to setup hooks again upon each connection
        log::error!(
            "Unable to initialize hooks, randomizer is unable to function: {:?}",
            status
        )
    });
}

pub(crate) static CLIENT: LazyLock<Mutex<Option<ArchipelagoClient>>> =
    LazyLock::new(|| Mutex::new(None));

pub(crate) static CONNECTION_STATUS: AtomicIsize = AtomicIsize::new(0); // Disconnected

#[tokio::main]
pub(crate) async fn spawn_arch_thread() {
    log::info!("Archipelago Thread started");

    CHECKLIST
        .set(RwLock::new(HashMap::new()))
        .expect("Unable to create the Checklist HashMap");
    let mut setup = false;
    let mut rx_locations = check_handler::setup_items_channel();
    let mut rx_connect = archipelago::setup_connect_channel();
    let mut rx_bank_to_inv = setup_bank_to_inv_channel();
    let mut rx_bank_add = setup_bank_add_channel();
    match load_login_data() {
        Ok(_) => {}
        Err(err) => log::error!("Unable to read login data: {}", err),
    }
    loop {
        // Wait for a connection request
        let Some(item) = rx_connect.recv().await else {
            log::warn!("Connect channel closed, exiting Archipelago thread.");
            break;
        };

        log::info!("Processing connection request: {}", item);
        let mut client_lock = CLIENT.lock().await;

        match connect_archipelago(item).await {
            Ok(cl) => {
                client_lock.replace(cl);
                CONNECTION_STATUS.store(Status::Connected.into(), Ordering::SeqCst);
                CHECKLIST.get().unwrap().write().unwrap().clear();
            }
            Err(err) => {
                log::error!("Failed to connect to Archipelago: {}", err);
                client_lock.take(); // Clear the client
                CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::SeqCst);
                SLOT_NUMBER.store(-1, Ordering::SeqCst);
                TEAM_NUMBER.store(-1, Ordering::SeqCst);
                continue; // Try again on next connection request
            }
        }

        // Client is successfully connected
        if let Some(ref mut client) = client_lock.as_mut() {
            if !setup {
                archipelago::run_setup(client).await;
                //item_sync::sync_items(client).await;
                setup = true;
            }
            if let Err(e) = client.status_update(ClientStatus::ClientReady).await {
                log::error!("Status update failed: {}", e);
            }
            // This blocks until a reconnect or disconnect is triggered
            archipelago::handle_things(
                client,
                &mut rx_locations,
                &mut rx_bank_to_inv,
                &mut rx_connect,
                &mut rx_bank_add,
            )
            .await;
        }
        CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::SeqCst);
        setup = false;
        // Allow reconnect immediately without delay
    }
}

fn load_login_data() -> Result<(), Box<dyn std::error::Error>> {
    if path::Path::new(archipelago::LOGIN_DATA_FILE).exists() {
        let login_data_file = fs::File::open(archipelago::LOGIN_DATA_FILE)?;
        let reader = BufReader::new(login_data_file);
        let mut json_reader = serde_json::Deserializer::from_reader(reader);
        let data = ArchipelagoData::deserialize(&mut json_reader)?;
        match ui::get_hud_data().lock() {
            Ok(mut instance) => {
                instance.archipelago_url = data.url;
                instance.username = data.name;
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    } else {
        Err("Failed to find login data".into())
    }
}

/// Set the starting gun and melee weapon upon a new game
pub unsafe fn set_starting_weapons(melee_id: u8, gun_id: u8) {
    unsafe {
        utilities::replace_single_byte(STARTING_MELEE, melee_id); // Melee weapon
        utilities::replace_single_byte(STARTING_GUN, gun_id); // Gun
        todo!()
    }
}

pub(crate) const DUMMY_ID: u8 = 0x20;

pub fn edit_event_drop(param_1: i64, param_2: i32, param_3: i64) {
    let (mapping, mission_event_tables, checked_locations) =
        if let (Ok(mapping), Some(mission_event_tables), Some(checked_locations)) = (
            get_mappings().lock(),
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
                        if checked_locations.contains(&event_table.location.to_string()) {
                            log::debug!("Event loc checked: {}", &event_table.location);
                            match event.event_type {
                                // If the location has already been checked use DUMMY_ID as a dummy item.
                                EventCode::GIVE => utilities::replace_single_byte_no_offset(
                                    EVENT_TABLE_ADDR + event.offset,
                                    DUMMY_ID,
                                ),
                                EventCode::CHECK => utilities::replace_single_byte_no_offset(
                                    EVENT_TABLE_ADDR + event.offset,
                                    DUMMY_ID,
                                ),
                                EventCode::END => utilities::replace_single_byte_no_offset(
                                    EVENT_TABLE_ADDR + event.offset,
                                    DUMMY_ID,
                                ),
                            }
                        } else {
                            log::debug!("Event loc not checked: {}", &event_table.location);
                            match event.event_type {
                                // Location has not been checked off! TODO Make the "check" event, a dummied out item
                                EventCode::GIVE => utilities::replace_single_byte_no_offset(
                                    EVENT_TABLE_ADDR + event.offset,
                                    get_item_id(
                                        &*mapping.items.get(event_table.location).unwrap().name,
                                    )
                                    .unwrap(),
                                ),
                                EventCode::CHECK => utilities::replace_single_byte_no_offset(
                                    EVENT_TABLE_ADDR + event.offset,
                                    DUMMY_ID,
                                ),
                                EventCode::END => utilities::replace_single_byte_no_offset(
                                    EVENT_TABLE_ADDR + event.offset,
                                    DUMMY_ID,
                                ),
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
// Set these from 01 to 02
pub(crate) unsafe fn rewrite_mode_table() {
    unsafe {
        let table_address = ITEM_MODE_TABLE + utilities::get_dmc3_base_address();
        let mut old_protect = 0;
        let length = 16;
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
pub(crate) unsafe fn modify_adjudicator_drop() {
    unsafe {
        match get_mappings().lock() {
            Ok(mapping) => {
                if let Some(mapping) = mapping.as_ref() {
                    for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
                        // Run through all locations
                        if entry.adjudicator && entry.mission == get_mission() {
                            // If Location is adjudicator and mission numbers match
                            let item_id =
                                get_item_id(&*mapping.items.get(*location_name).unwrap().name)
                                    .unwrap(); // Get the item ID and replace
                            utilities::replace_single_byte(ADJUDICATOR_ITEM_ID_1, item_id);
                            utilities::replace_single_byte(ADJUDICATOR_ITEM_ID_2, item_id);
                        }
                    }
                }
            }
            _ => {}
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
        log::debug!("Item count: {:x}", item_count);
        let room_num: u16 = utilities::get_room() as u16;
        //set_relevant_key_items();
        match get_mappings().lock() {
            Ok(mapping) => match mapping.as_ref() {
                None => {
                    log::warn!("Mapping's are not set up. Logging debug info:");
                    for _i in 0..item_count {
                        let item_ref: &u32 = &*(item_addr as *const u32);
                        log::debug!(
                            "Item ID: {} (0x{:x})",
                            get_item_name(*item_ref as u8),
                            *item_ref
                        );
                        item_addr = item_addr.byte_offset(0x14);
                    }
                }
                Some(mapping) => {
                    modify_adjudicator_drop();
                    modify_secret_mission_item();
                    for _i in 0..item_count {
                        let item_ref: &u32 = &*(item_addr as *const u32);
                        log::debug!(
                            "Item ID: {} (0x{:x})",
                            get_item_name(*item_ref as u8),
                            *item_ref
                        );
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

fn modify_secret_mission_item() {
    unsafe {
        match get_mappings().lock() {
            Ok(mapping) => {
                match mapping.as_ref() {
                    Some(mapping) => {
                        for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
                            // Run through all locations
                            if get_room() == entry.room_number as i32 {
                                // Only on
                                let item_id =
                                    get_item_id(&*mapping.items.get(*location_name).unwrap().name).unwrap(); // Get the item ID and replace
                                utilities::replace_single_byte(SECRET_MISSION_ITEM, item_id);
                            }
                        }
                    }
                    None => log::warn!("Attempted to modify secret mission item without mappings"),
                }
            }
            _ => {}
        }
    }
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
                let ins_val = get_item_id(&*mapping.items.get(*location_name).unwrap().name); // Scary

                if ITEM_MAP.get(&entry.item_id).unwrap().category == ItemCategory::Key {
                    *item_addr = 0x26i32;
                } else {
                    *item_addr = ins_val.unwrap() as i32;
                }
                log::info!(
                    "Replaced item in room {} ({}) with {} 0x{:x}",
                    entry.room_number,
                    location_name,
                    get_item_name(*item_ref as u8),
                    *item_addr
                );
            }
        }
    }
}

fn dummy_replace(location_key: &&str, item_addr: *mut i32, _offset: usize) -> bool {
    match EVENT_TABLES.get(&get_mission()) {
        None => {}
        Some(event_tables) => {
            for event_table in event_tables {
                if event_table.location == *location_key {
                    for event in event_table.events.iter() {
                        if event.event_type == EventCode::END {
                            unsafe {
                                match archipelago::get_checked_locations().lock() {
                                    Ok(checked_locations) => {
                                        if checked_locations.contains(&String::from(*location_key))
                                        {
                                            *item_addr = DUMMY_ID as i32;
                                            // modify_itm_table(offset, DUMMY_ID);
                                            log::info!("Replaced item in room with dummy item");
                                            return true;
                                        }
                                    }
                                    Err(err) => {
                                        log::error!(
                                            "Failed to lock checked locations vec: {}",
                                            err
                                        );
                                    }
                                }
                            }
                        }
                    }
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
    let checklist: RwLockWriteGuard<HashMap<String, bool>> =
        CHECKLIST.get().unwrap().write().unwrap();
    let current_inv_addr = utilities::read_usize_from_address(INVENTORY_PTR);
    log::debug!("Current INV Addr: 0x{:x}", current_inv_addr);
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
                    current_inv_addr + ITEM_OFFSET_MAP.get(item).unwrap().clone() as usize;
                log::debug!(
                    "Attempting to replace at address: 0x{:x} with flag 0x{:x}",
                    item_addr,
                    flag
                );
                unsafe { utilities::replace_single_byte_no_offset(item_addr, flag) };
            }
        }
    }
}

macro_rules! install_hook {
    ($offset:expr, $detour:expr, $storage:ident, $name:expr) => {{
        let target = (utilities::get_dmc3_base_address() + $offset) as *mut _;
        let detour_ptr = ($detour as *const ()) as *mut std::ffi::c_void;
        let original = MinHook::create_hook(target, detour_ptr)?;
        $storage
            .set(std::mem::transmute(original))
            .expect(concat!($name, " hook already set"));
        MinHook::enable_hook(target)?;
        log::info!("{name} hook enabled", name = $name);
    }};
}

// 23d680 - Pause menu event? Hook in here to do rendering
fn setup_hooks() -> Result<(), MH_STATUS> {
    unsafe {
        install_hook!(
            ITEM_HANDLE_PICKUP_ADDR,
            check_handler::item_non_event,
            ORIGINAL_HANDLE_PICKUP,
            "Non event item"
        );
        install_hook!(
            ITEM_PICKED_UP_ADDR,
            check_handler::item_event,
            ORIGINAL_ITEM_PICKED_UP,
            "Event item"
        );
        install_hook!(
            RESULT_SCREEN_ADDR,
            check_handler::mission_complete_check,
            ORIGINAL_HANDLE_MISSION_COMPLETE,
            "Mission complete"
        );

        install_hook!(
            ITEM_SPAWNS_ADDR,
            item_spawns_hook,
            ORIGINAL_ITEM_SPAWNS,
            "Item Spawn"
        );
        install_hook!(
            EDIT_EVENT_HOOK,
            edit_event_drop,
            ORIGINAL_EDIT_EVENT,
            "Event table"
        );
        install_hook!(
            0x23a7b0usize,
            setup_inventory_for_mission,
            ORIGINAL_MISSION_INV,
            "Setup mission inventory"
        );
        // install_hook!(
        //     RENDER_TEXT_ADDR,
        //     parry_text,
        //     ORIGINAL_RENDER_TEXT,
        //     "Render Text"
        // );
        Ok(())
    }
}

/// Disable hooks, used for disconnecting
pub fn disable_hooks() -> Result<(), MH_STATUS> {
    let base_address = utilities::get_dmc3_base_address();
    unsafe {
        MinHook::disable_hook((base_address + ITEM_HANDLE_PICKUP_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + ITEM_PICKED_UP_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + RESULT_SCREEN_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + ITEM_SPAWNS_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + EDIT_EVENT_HOOK) as *mut _)?;
        MinHook::disable_hook((base_address + 0x23a7b0usize) as *mut _)?;
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
    res
}

pub static CANCEL_TEXT: AtomicBool = AtomicBool::new(false);

pub unsafe fn parry_text(
    param_1: c_longlong,
    param_2: c_longlong,
    param_3: c_longlong,
    param_4: c_longlong,
) {
    //Parry text: param_1: 7ff7648d89a0, param_2: 120, param_3: 4140, param_4: 10000,
    // Parry text: param_1: 7ff7648d89a0, param_2: 120, param_3: 140, param_4: 10000,
    if param_1 == (utilities::get_dmc3_base_address() + 0xCB89A0) as c_longlong {
        // This might only be compatible with ENG?
        log::debug!(
            "Parry text: param_1: {:x}, param_2: {:x}, param_3: {:x}, param_4: {:x},",
            param_1,
            param_2,
            param_3,
            param_4
        );
        if CANCEL_TEXT.load(Ordering::Relaxed) {
            CANCEL_TEXT.store(false, Ordering::Relaxed);
            return;
        }
    }
    unsafe {
        if let Some(original) = ORIGINAL_RENDER_TEXT.get() {
            original(param_1, param_2, param_3, param_4);
        }
    }
}

/// The mapping data at dmc3.exe+5c4c20+1A00
/// This table dictates what item is in what room. Only relevant for consumables and blue orb fragments
pub unsafe fn modify_item_table(offset: usize, id: u8) {
    unsafe {
        // let start_addr = 0x5C4C20usize; dmc3.exe+5c4c20+1A00
        // let end_addr = 0x5C4C20 + 0xC8; // 0x5C4CE8
        let true_offset = offset + utilities::get_dmc3_base_address() + 0x1A00usize; // MFW I can't do my offsets correctly
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
