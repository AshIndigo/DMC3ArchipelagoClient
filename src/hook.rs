use crate::archipelago::get_checked_locations;
use crate::constants::ItemEntry;
use crate::constants::*;
use crate::data::generated_locations;
use crate::game_manager::{get_mission, get_room, set_item, set_loc_chk_flg, set_weapons_in_inv};
use crate::location_handler::in_key_item_room;
use crate::mapping::{Mapping, MAPPING};
use crate::ui::ui::{CHECKLIST, CONNECTION_STATUS};
use crate::utilities::{read_data_from_address, replace_single_byte, DMC3_ADDRESS};
use crate::{check_handler, create_hook, game_manager, save_handler, text_handler, utilities};
use archipelago_rs::client::ArchipelagoClient;
use archipelago_rs::protocol::{Bounce, ClientMessage};
use log::error;
use minhook::{MinHook, MH_STATUS};
use serde_json::json;
use std::arch::asm;
use std::collections::HashMap;
use std::ptr::read_unaligned;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, OnceLock, RwLockReadGuard};
use std::{ptr, slice, thread};
use tokio::sync::Mutex;
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::PAGE_EXECUTE_READWRITE;

pub(crate) const DUMMY_ID: LazyLock<u32> = LazyLock::new(|| *ITEM_ID_MAP.get("Dummy").unwrap()); //0x20;
pub(crate) const REMOTE_ID: LazyLock<u32> = LazyLock::new(|| *ITEM_ID_MAP.get("Remote").unwrap()); //0x26;

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
        check_handler::setup_check_hooks()?;
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
            LOAD_NEW_ROOM_ADDR,
            load_new_room,
            ORIGINAL_LOAD_NEW_ROOM,
            "Load new room"
        );
        create_hook!(
            SETUP_PLAYER_DATA_ADDR,
            set_player_data,
            ORIGINAL_SETUP_PLAYER_DATA,
            "Setup player data"
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
        create_hook!(
            ADJUDICATOR_DATA_ADDR,
            modify_adjudicator,
            ORIGINAL_ADJUDICATOR_DATA,
            "Modify Adjudicator Data"
        );
        text_handler::setup_text_hooks()?;
        save_handler::setup_save_hooks()?;
    }
    Ok(())
}

fn enable_hooks() {
    let addresses: Vec<usize> = vec![
        // Check handling
        check_handler::ITEM_HANDLE_PICKUP_ADDR,
        check_handler::ITEM_PICKED_UP_ADDR,
        check_handler::RESULT_CALC_ADDR,
        // Misc
        ITEM_SPAWNS_ADDR, // Handles replacing item spawns.
        EDIT_EVENT_HOOK_ADDR,
        LOAD_NEW_ROOM_ADDR,
        EQUIPMENT_SCREEN_ADDR,
        SETUP_PLAYER_DATA_ADDR,
        DAMAGE_CALC_ADDR,
        ADJUDICATOR_DATA_ADDR,
        // Save handler
        save_handler::LOAD_GAME_ADDR,
        save_handler::SAVE_GAME_ADDR,
        // Text Handler
        text_handler::DISPLAY_ITEM_GET_ADDR,
        text_handler::DISPLAY_ITEM_GET_DESTRUCTOR_ADDR,
        text_handler::SETUP_ITEM_GET_SCREEN_ADDR,
    ];
    addresses.iter().for_each(|addr| unsafe {
        match MinHook::enable_hook((*DMC3_ADDRESS + addr) as *mut _) {
            Ok(_) => {}
            Err(err) => {
                log::error!("Failed to enable {:#X} hook: {:?}", addr, err);
            }
        }
    })
}

pub(crate) static CLIENT: LazyLock<Mutex<Option<ArchipelagoClient>>> =
    LazyLock::new(|| Mutex::new(None));

pub fn edit_event_drop(param_1: usize, param_2: i32, param_3: usize) {
    let (mapping, mission_event_tables, checked_locations) =
        if let (Ok(mapping), Some(mission_event_tables), Some(checked_locations)) = (
            MAPPING.read(),
            EVENT_TABLES.get(&get_mission()),
            get_checked_locations().read().ok(),
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
        Some(_mapping) => {
            unsafe {
                // For each table
                for event_table in mission_event_tables {
                    for event in &event_table.events {
                        if checked_locations.contains(&event_table.location) {
                            log::debug!("Event loc checked: {}", &event_table.location);
                            match event.event_type {
                                // If the location has already been checked use DUMMY_ID as a dummy item.
                                EventCode::GIVE => replace_single_byte(
                                    EVENT_TABLE_ADDR + event.offset,
                                    *DUMMY_ID as u8,
                                ),
                                EventCode::CHECK => replace_single_byte(
                                    EVENT_TABLE_ADDR + event.offset,
                                    *DUMMY_ID as u8,
                                ),
                                EventCode::END => replace_single_byte(
                                    EVENT_TABLE_ADDR + event.offset,
                                    *DUMMY_ID as u8,
                                ),
                            }
                        } else {
                            log::debug!("Event loc not checked: {}", &event_table.location);
                            match event.event_type {
                                // Location has not been checked off!
                                EventCode::GIVE => log::debug!("Give event")/*replace_single_byte(
                                    EVENT_TABLE_ADDR + event.offset,
                                    get_item_id(
                                        &*mapping
                                            .items
                                            .get(event_table.location)
                                            .unwrap()
                                            .item_name,
                                    )
                                    .unwrap(),
                                )*/,
                                EventCode::CHECK => {
                                    log::debug!("Replaced check at {:#X}", &event.offset);
                                    replace_single_byte(EVENT_TABLE_ADDR + event.offset, *DUMMY_ID as u8)
                                }
                                EventCode::END => {
                                    log::debug!("Replaced end at {:#X}", &event.offset);
                                    replace_single_byte(EVENT_TABLE_ADDR + event.offset, *DUMMY_ID as u8)
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
    let table_address = ITEM_MODE_TABLE + *DMC3_ADDRESS;
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

/// Modify adjudicator weapon and rank info
fn modify_adjudicator(param_1: usize, param_2: usize, param_3: usize, adjudicator_data: usize) -> usize {
    const LOG_ADJU_DATA: bool = true;
    const RANKING_OFFSET: usize = 0x04;
    const WEAPON_OFFSET: usize = 0x06;
    for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
        // Run through all locations
        if entry.adjudicator && entry.mission == get_mission() {
            if LOG_ADJU_DATA {
                log::debug!("Adjudicator found at location {}", &location_name);
                log::debug!(
                    "Rank Needed: {}",
                    Rank::from_repr(
                        read_data_from_address::<u8>(adjudicator_data + RANKING_OFFSET) as usize
                            - 1
                    )
                    .expect(
                        format!(
                            "Unable to get rank from adjudicator: {}",
                            read_data_from_address::<u8>(adjudicator_data + RANKING_OFFSET)
                        )
                        .as_str()
                    )
                );
                log::debug!(
                    "Melee: {}",
                    read_data_from_address::<u8>(adjudicator_data + WEAPON_OFFSET)
                );
            }
            unsafe {
                match MAPPING.read().as_ref() {
                    Ok(mapping_opt) => match &**mapping_opt {
                        Some(mappings) => {
                            let data = mappings.adjudicators.get(*location_name).unwrap();
                            replace_single_byte(
                                adjudicator_data + RANKING_OFFSET,
                                data.ranking + 1,
                            );
                            replace_single_byte(
                                adjudicator_data + WEAPON_OFFSET,
                                get_weapon_id(&*data.weapon),
                            );
                        }
                        None => {}
                    },
                    Err(err) => {
                        log::error!("Failed to read mapping: {:?}", err);
                    }
                }
            }
            if LOG_ADJU_DATA {
                log::debug!(
                    "New Rank Needed: {}",
                    Rank::from_repr(
                        read_data_from_address::<u8>(adjudicator_data + RANKING_OFFSET) as usize
                            - 1
                    )
                    .unwrap()
                );
                log::debug!(
                    "New Melee: {}",
                    read_data_from_address::<u8>(adjudicator_data + WEAPON_OFFSET)
                );
            }
        }
    }
    if let Some(original) = ORIGINAL_ADJUDICATOR_DATA.get() {
        unsafe { original(param_1, param_2, param_3, adjudicator_data) }
    } else {
        panic!("Could not find original adjudicator method")
    }
}

fn item_spawns_hook(unknown: usize) {
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
        let room_num = get_room();
        log::debug!("Room num: {} ({:#X})", room_num, room_num);
        match MAPPING.read() {
            Ok(mapping) => match mapping.as_ref() {
                None => {
                    log::warn!("Mapping's are not set up. Logging debug info:");
                    for _i in 0..item_count {
                        let item_ref: &u32 = &*(item_addr as *const u32);
                        log::debug!("Item ID: {} ({:#X})", get_item_name(*item_ref), *item_ref);
                        item_addr = item_addr.byte_offset(0x14);
                    }
                }
                Some(mapping) => {
                    //modify_adjudicator_drop(mapping);
                    // modify_secret_mission_item(mapping);
                    for _i in 0..item_count {
                        let item_ref: &u32 = &*(item_addr as *const u32);
                        const EXTRA_OUTPUT: bool = true;
                        if EXTRA_OUTPUT {
                            log::debug!("Item ID: {} ({:#X})", get_item_name(*item_ref), *item_ref);
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

fn monitor_hp(damage_calc: usize, param_1: usize, param_2: usize, param_3: usize) {
    unsafe { ORIGINAL_DAMAGE_CALC.get().unwrap()(damage_calc, param_1, param_2, param_3) }
    let char_data_ptr: usize =
        read_data_from_address(*DMC3_ADDRESS + game_manager::ACTIVE_CHAR_DATA);
    unsafe {
        let hp = read_unaligned((char_data_ptr + 0x411C) as *mut f32);
        if hp == 0.0 {
            log::debug!("Dante died!");
            match MAPPING.read() {
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
/// Edits the initially selected index when viewing weapons in the status screen
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
    let base = *DMC3_ADDRESS;
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
    room_num: i32,
    _mapping: &Mapping,
    item_ref: &u32,
    item_addr: *mut i32,
) {
    unsafe {
        // Skipping if location file has room as 0, that means its either event or not done
        if entry.room_number == 0 {
            return;
        }
        //log::debug!("Room number X: {} Room number memory: {}, Item ID X: {:#X}, Item ID Memory: {:#X}", entry.room_number, room_num, entry.item_id, *item_ref);
        if entry.room_number == room_num && entry.item_id == *item_ref && !entry.adjudicator {
            log::debug!("Seeing if item needs to be dummy");
            if !dummy_replace(location_name, item_addr) {
                //let ins_val = get_item_id(&*mapping.items.get(*location_name).unwrap().item_name); // Scary

                if ITEM_MAP.get(&entry.item_id).unwrap().category == ItemCategory::Key {
                    //*item_addr = *(ITEM_ID_MAP.get("Remote").unwrap()) as i32; // 0x26 is high roller card, what is used for remote items.
                } else {
                    //*item_addr = ins_val.unwrap() as i32;
                }
                log::info!(
                    "Replaced item in room {} ({}) with {} {:#X}",
                    entry.room_number,
                    location_name,
                    get_item_name(*item_ref),
                    *item_addr
                );
            }
        }
    }
}

/// Replaces an item with a dummy one in order to not immediately proc end events upon entering the location's room
fn dummy_replace(location_key: &&str, item_addr: *mut i32) -> bool {
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
                if let Ok(checked_locations) = get_checked_locations().read() {
                    if checked_locations.contains(location_key) {
                        unsafe {
                            *item_addr = *DUMMY_ID as i32;
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
    game_manager::with_session(|s| {
        log::debug!("Current mission: {}", s.mission);
        match MISSION_ITEM_MAP.get(&(s.mission)) {
            None => {} // No items for the mission
            Some(item_list) => {
                for item in item_list.into_iter() {
                    if *checklist.get(*item).unwrap_or(&false) {
                        let res = game_manager::has_item_by_flags(item);
                        if !res {
                            set_item(item, true, true);
                        }
                        log::debug!("Item Relevant to mission {} Flag: {}", *item, res);
                    } else {
                        set_item(item, false, true);
                    }
                }
            }
        }
        match MISSION_ITEM_MAP.get(&(s.mission)) {
            None => {} // No items for the mission
            Some(item_list) => {
                for item in item_list.into_iter() {
                    if *checklist.get(*item).unwrap_or(&false) {
                        let res = game_manager::has_item_by_flags(item);
                        if !res {
                            set_loc_chk_flg(item, true)
                        }
                    }
                }
            }
        }
        if let Ok(loc) = in_key_item_room() {
            log::debug!("In key room: {}", loc);
            if get_checked_locations().read().unwrap().contains(&loc) {
                //set_loc_chk_flg(get_item_name(generated_locations::ITEM_MISSION_MAP.get(loc).unwrap().item_id), true);
            } else {
                set_loc_chk_flg(
                    get_item_name(
                        generated_locations::ITEM_MISSION_MAP
                            .get(loc)
                            .unwrap()
                            .item_id,
                    ),
                    false,
                );
            }
        }
    })
    .unwrap();
}

/// Disable hooks, used for disconnecting
pub fn disable_hooks() -> Result<(), MH_STATUS> {
    let base_address = *DMC3_ADDRESS;
    unsafe {
        MinHook::disable_hook((base_address + ITEM_SPAWNS_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + EDIT_EVENT_HOOK_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + LOAD_NEW_ROOM_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + SETUP_PLAYER_DATA_ADDR) as *mut _)?;
        save_handler::disable_save_hooks(base_address)?;
        MinHook::disable_hook((base_address + EQUIPMENT_SCREEN_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + DAMAGE_CALC_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + ADJUDICATOR_DATA_ADDR) as *mut _)?;
        text_handler::disable_text_hooks(base_address)?;
        check_handler::disable_check_hooks(base_address)?;
    }
    Ok(())
}

pub const LOAD_NEW_ROOM_ADDR: usize = 0x23e610;

pub static ORIGINAL_LOAD_NEW_ROOM: OnceLock<unsafe extern "C" fn(param_1: usize) -> bool> =
    OnceLock::new();

pub fn load_new_room(param_1: usize) -> bool {
    let mut res = false;
    unsafe {
        if let Some(original) = ORIGINAL_LOAD_NEW_ROOM.get() {
            res = original(param_1);
        }
    }
    set_relevant_key_items();
    set_weapons_in_inv();
    check_handler::clear_high_roller();
    //location_handler::room_transition();
    res
}

pub const SETUP_PLAYER_DATA_ADDR: usize = 0x23a7b0;

pub static ORIGINAL_SETUP_PLAYER_DATA: OnceLock<unsafe extern "C" fn(param_1: usize) -> bool> =
    OnceLock::new();

pub fn set_player_data(param_1: usize) -> bool {
    let mut res = false;
    game_manager::set_session_weapons();
    game_manager::set_max_hp_and_magic();

    unsafe {
        if let Some(original) = ORIGINAL_SETUP_PLAYER_DATA.get() {
            res = original(param_1)
        } else {
            log::error!("No setup player data found");
        }
    }
    res
}

/// The mapping data at dmc3.exe+5c4c20+1A00
/// This table dictates what item is in what room. Only relevant for consumables and blue orb fragments
pub fn modify_item_table(offset: usize, id: u8) {
    unsafe {
        // let start_addr = 0x5C4C20usize; dmc3.exe+5c4c20+1A00
        // let end_addr = 0x5C4C20 + 0xC8; // 0x5C4CE8
        let true_offset = offset + *DMC3_ADDRESS + 0x1A00usize; // MFW I can't do my offsets correctly
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
                let true_offset = val.offset + *DMC3_ADDRESS + 0x1A00usize;
                let mut old_protect = 0;
                VirtualProtect(
                    true_offset as *mut _,
                    4, // Length of table I need to modify
                    PAGE_EXECUTE_READWRITE,
                    &mut old_protect,
                );

                let table = slice::from_raw_parts_mut(true_offset as *mut u8, 4);

                table[3] = val.item_id as u8;

                VirtualProtect(true_offset as *mut _, 4, old_protect, &mut old_protect);
            }
        })
}

/// Set the modified modes back to 1 from 2
pub(crate) fn restore_mode_table() {
    let table_address = ITEM_MODE_TABLE + *DMC3_ADDRESS;
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
