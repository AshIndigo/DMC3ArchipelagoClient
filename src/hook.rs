use crate::archipelago::{DeathLinkData, CHECKED_LOCATIONS, TX_DEATHLINK};
use crate::constants::ItemEntry;
use crate::constants::*;
use crate::data::generated_locations;
use crate::game_manager::{get_difficulty, get_mission, get_room, set_item, set_loc_chk_flg, set_weapons_in_inv, Style, ARCHIPELAGO_DATA};
use crate::location_handler::in_key_item_room;
use crate::mapping::{Mapping, MAPPING};
use crate::ui::text_handler;
use crate::ui::text_handler::LAST_OBTAINED_ID;
use crate::ui::ui::CONNECTION_STATUS;
use crate::utilities::{read_data_from_address, replace_single_byte, DMC3_ADDRESS};
use crate::{bank, check_handler, create_hook, game_manager, mapping, save_handler, skill_manager, utilities};
use archipelago_rs::client::ArchipelagoClient;
use minhook::{MinHook, MH_STATUS};
use std::arch::asm;
use std::ptr::{read_unaligned, write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::{ptr, slice};
use tokio::sync::Mutex;
use windows::Win32::System::Memory::{
    VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
};

pub(crate) static CLIENT: LazyLock<Mutex<Option<ArchipelagoClient>>> =
    LazyLock::new(|| Mutex::new(None));

static HOOKS_CREATED: AtomicBool = AtomicBool::new(false);

pub(crate) fn install_initial_functions() {
    if !HOOKS_CREATED.load(Ordering::SeqCst) {
        unsafe {
            match create_hooks() {
                Ok(_) => {
                    HOOKS_CREATED.store(true, Ordering::SeqCst);
                }
                Err(err) => {
                    log::error!("Failed to create hooks: {:?}", err);
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
        create_hook!(
            SKILL_SHOP_ADDR,
            deny_skill_purchasing,
            ORIGINAL_SKILL_SHOP,
            "Deny purchases of skills"
        );
        create_hook!(
            GUN_SHOP_ADDR,
            deny_gun_upgrade,
            ORIGINAL_GUN_SHOP,
            "Deny purchasing gun upgrades"
        );
        create_hook!(
            ADD_SHOTGUN_OR_CERBERUS_ADDR,
            deny_cerberus_or_shotgun,
            ORIGINAL_ADD_SHOTGUN_OR_CERBERUS,
            "Don't add the Shotgun/Cerberus to second slot"
        );
        create_hook!(
            CUSTOMIZE_STYLE_MENU,
            modify_available_styles,
            ORIGINAL_STYLE_MENU,
            "Hide styles that aren't available"
        );
        create_hook!(
            GIVE_STYLE_XP,
            give_no_xp,
            ORIGINAL_GIVE_STYLE_XP,
            "Don't give style XP"
        );
        create_hook!(
            SET_NEW_SESSION_DATA,
            set_rando_session_data,
            ORIGINAL_SET_NEW_SESSION_DATA,
            "Set session data for new game"
        );
        text_handler::setup_text_hooks()?;
        save_handler::setup_save_hooks()?;
        bank::setup_bank_hooks()?;
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
        SKILL_SHOP_ADDR,
        GUN_SHOP_ADDR,
        ADD_SHOTGUN_OR_CERBERUS_ADDR,
        CUSTOMIZE_STYLE_MENU,
        GIVE_STYLE_XP,
        SET_NEW_SESSION_DATA,
        // Bank
        bank::OPEN_INV_SCREEN_ADDR,
        bank::CLOSE_INV_SCREEN_ADDR,
        bank::USE_ITEM_ADDR,
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
        MinHook::disable_hook((base_address + SKILL_SHOP_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + GUN_SHOP_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + ADD_SHOTGUN_OR_CERBERUS_ADDR) as *mut _)?;
        MinHook::disable_hook((base_address + CUSTOMIZE_STYLE_MENU) as *mut _)?;
        MinHook::disable_hook((base_address + SET_NEW_SESSION_DATA) as *mut _)?;
        text_handler::disable_text_hooks(base_address)?;
        check_handler::disable_check_hooks(base_address)?;
        bank::disable_bank_hooks(base_address)?;
    }
    Ok(())
}

pub const EDIT_EVENT_HOOK_ADDR: usize = 0x1a9bc0;
pub static ORIGINAL_EDIT_EVENT: OnceLock<
    unsafe extern "C" fn(param_1: usize, param_2: i32, param_3: usize),
> = OnceLock::new();
pub fn edit_event_drop(param_1: usize, param_2: i32, param_3: usize) {
    let (mapping, mission_event_tables, checked_locations) =
        if let (Ok(mapping), Some(mission_event_tables), Some(checked_locations)) = (
            MAPPING.read(),
            EVENT_TABLES.get(&get_mission()),
            CHECKED_LOCATIONS.read().ok(),
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
                        if let Some(event_table_addr) = utilities::get_event_address() {
                            if checked_locations.contains(&event_table.location) {
                                log::debug!("Event loc checked: {}", &event_table.location);
                                match event.event_type {
                                    // If the location has already been checked use DUMMY_ID as a dummy item.
                                    EventCode::GIVE => replace_single_byte(
                                        event_table_addr + event.offset,
                                        *DUMMY_ID as u8,
                                    ),
                                    EventCode::CHECK => replace_single_byte(
                                        event_table_addr + event.offset,
                                        *DUMMY_ID as u8,
                                    ),
                                    EventCode::END => replace_single_byte(
                                        event_table_addr + event.offset,
                                        *DUMMY_ID as u8,
                                    ),
                                }
                            } else {
                                log::debug!("Event loc not checked: {}", &event_table.location);
                                match event.event_type {
                                    // Location has not been checked off!
                                    EventCode::GIVE => {},
                                    EventCode::CHECK => {
                                        log::debug!("Replaced check at {:#X}", &event.offset);
                                        replace_single_byte(event_table_addr + event.offset, *DUMMY_ID as u8)
                                    }
                                    EventCode::END => {
                                        log::debug!("Replaced end at {:#X}", &event.offset);
                                        replace_single_byte(event_table_addr + event.offset, *DUMMY_ID as u8)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    unsafe {
        if let Some(original) = ORIGINAL_EDIT_EVENT.get() {
            original(param_1, param_2, param_3);
        }
    }
}

/// Modify the game's code so the "pickup mode" table is correct
// start at 1B3944 -> 1B395A
// Set these from 02 to 01
pub(crate) fn rewrite_mode_table() {
    let table_address = ITEM_MODE_TABLE + *DMC3_ADDRESS;
    let mut old_protect = PAGE_PROTECTION_FLAGS::default();
    let length = 16;
    unsafe {
        VirtualProtect(
            table_address as *mut _,
            length, // Length of table I need to modify
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
        .expect("Unable to rewrite mode table - Before");

        let table = slice::from_raw_parts_mut(table_address as *mut u8, length);
        table.fill(0x01u8); // 0 = orbs, 1 = items, 2 = bad

        VirtualProtect(
            table_address as *mut _,
            length,
            old_protect,
            &mut old_protect,
        )
        .expect("Unable to rewrite mode table - After");
    }
}

pub const ADJUDICATOR_DATA_ADDR: usize = 0x24f970;
pub static ORIGINAL_ADJUDICATOR_DATA: OnceLock<
    unsafe extern "C" fn(param_1: usize, param_2: usize, param_3: usize, param_4: usize) -> usize,
> = OnceLock::new();
/// Modify adjudicator weapon and rank info
fn modify_adjudicator(
    param_1: usize,
    param_2: usize,
    param_3: usize,
    adjudicator_data: usize,
) -> usize {
    const LOG_ADJU_DATA: bool = true;
    const RANKING_OFFSET: usize = 0x04;
    const WEAPON_OFFSET: usize = 0x06;
    for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
        // Run through all locations
        if entry.adjudicator && entry.room_number == get_room() {
            //&& entry.mission == get_mission() {
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
                            replace_single_byte(adjudicator_data + RANKING_OFFSET, data.ranking);
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
    log::debug!("Calling original adjudicator method");
    if let Some(original) = ORIGINAL_ADJUDICATOR_DATA.get() {
        unsafe {
            let res = original(param_1, param_2, param_3, adjudicator_data);
            log::debug!("Finished adjudicator hook");
            res
        }
    } else {
        panic!("Could not find original adjudicator method")
    }
}

pub const ITEM_SPAWNS_ADDR: usize = 0x1b4440; // 0x1b4480
pub static ORIGINAL_ITEM_SPAWNS: OnceLock<unsafe extern "C" fn(loc_chk_id: usize)> =
    OnceLock::new();
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

pub const DAMAGE_CALC_ADDR: usize = 0x088190;
pub static ORIGINAL_DAMAGE_CALC: OnceLock<
    unsafe extern "C" fn(damage_calc: usize, param_1: usize, param_2: usize, param_3: usize),
> = OnceLock::new();

fn monitor_hp(damage_calc: usize, param_1: usize, param_2: usize, param_3: usize) {
    unsafe { ORIGINAL_DAMAGE_CALC.get().unwrap()(damage_calc, param_1, param_2, param_3) }
    if let Some(char_data_ptr) = utilities::get_active_char_address() {
        unsafe {
            let hp = read_unaligned((char_data_ptr + 0x411C) as *mut f32);
            if hp <= 0.0 {
                log::debug!("Dante died!");
                send_deathlink();
            }
        }
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn send_deathlink() {
    TX_DEATHLINK
        .get()
        .unwrap()
        .send(DeathLinkData {
            cause: format!(
                "{} died in Mission #{} on {}", // TODO Maybe an "against {}" at some point?
                mapping::get_own_slot_name().unwrap(),
                get_mission(),
                get_difficulty()
            ),
        })
        .await
        .unwrap();
}

pub const EQUIPMENT_SCREEN_ADDR: usize = 0x28CBD0;
pub static ORIGINAL_EQUIPMENT_SCREEN: OnceLock<unsafe extern "C" fn(cuid_weapon: usize) -> i32> =
    OnceLock::new();
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
                if let Ok(checked_locations) = CHECKED_LOCATIONS.read() {
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
    let current_inv_addr = utilities::get_inv_address();
    if current_inv_addr.is_none() {
        return;
    }
    log::debug!("Current INV Addr: {:#X}", current_inv_addr.unwrap());
    if let Ok(data) = ARCHIPELAGO_DATA.read() {
        game_manager::with_session(|s| {
            log::debug!("Current mission: {}", s.mission);
            match MISSION_ITEM_MAP.get(&(s.mission)) {
                None => {} // No items for the mission
                Some(item_list) => {
                    for item in item_list.into_iter() {
                        if data.items.contains(item) {
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
                        if data.items.contains(item)  {
                            let res = game_manager::has_item_by_flags(item);
                            if !res {
                                set_loc_chk_flg(item, true)
                            }
                        }
                    }
                }
            }
            // Special case for Ignis Fatuus
            // Needed so the Ignis Fatuus location can be reached even when the actual key item is acquired
            if get_room() == 302 {
                if let Some(event_table_addr) = utilities::get_event_address() {
                    if CHECKED_LOCATIONS
                        .read()
                        .unwrap()
                        .contains(&"Mission #8 - Ignis Fatuus")
                    {
                        // If we have the location checked, continue normal routing
                        unsafe {
                            write((event_table_addr + 0x748) as _, 311);
                        }
                    } else {
                        // If location not checked, alter event to get to it
                        unsafe {
                            write((event_table_addr + 0x748) as _, 303);
                        }
                    }
                }
            }

            if let Ok(loc) = in_key_item_room() {
                log::debug!("In key room: {}", loc);
                if CHECKED_LOCATIONS.read().unwrap().contains(&loc) {
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
    LAST_OBTAINED_ID.store(0, Ordering::SeqCst); // Should stop random item jumpscares
    skill_manager::set_skills(&ARCHIPELAGO_DATA.read().unwrap());
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
    if let Some(mapping) = MAPPING.read().unwrap().as_ref() {
        if mapping.randomize_skills {
            game_manager::set_gun_levels();
            skill_manager::set_skills(&ARCHIPELAGO_DATA.read().unwrap());
        }
        if mapping.randomize_styles {
            game_manager::set_style_levels()
        }
    }
    LAST_OBTAINED_ID.store(0, Ordering::SeqCst); // Should stop random item jumpscares
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
        let mut old_protect = PAGE_PROTECTION_FLAGS::default();
        VirtualProtect(
            true_offset as *mut _,
            4, // Length of table I need to modify
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
        .expect("Unable to modify item table - Before");

        let table = slice::from_raw_parts_mut(true_offset as *mut u8, 4);

        table[3] = id;

        VirtualProtect(true_offset as *mut _, 4, old_protect, &mut old_protect)
            .expect("Unable to modify item table - After");
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
                let mut old_protect = PAGE_PROTECTION_FLAGS::default();
                VirtualProtect(
                    true_offset as *mut _,
                    4, // Length of table I need to modify
                    PAGE_EXECUTE_READWRITE,
                    &mut old_protect,
                )
                .expect("Unable to restore item table - Before");

                let table = slice::from_raw_parts_mut(true_offset as *mut u8, 4);

                table[3] = val.item_id as u8;

                VirtualProtect(true_offset as *mut _, 4, old_protect, &mut old_protect)
                    .expect("Unable to restore item table - After");
            }
        })
}

/// Set the modified modes back to 1 from 2
pub(crate) fn restore_mode_table() {
    let table_address = ITEM_MODE_TABLE + *DMC3_ADDRESS;
    let mut old_protect = PAGE_PROTECTION_FLAGS::default();
    let length = 16;
    unsafe {
        VirtualProtect(
            table_address as *mut _,
            length, // Length of table I need to modify
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
        .expect("Unable to restore mode table - Before");

        let table = slice::from_raw_parts_mut(table_address as *mut u8, length);
        table.fill(0x02u8); // 0 = orbs, 1 = items, 2 = bad

        VirtualProtect(
            table_address as *mut _,
            length,
            old_protect,
            &mut old_protect,
        )
        .expect("Unable to restore mode table - After");
    }
}

pub const SKILL_SHOP_ADDR: usize = 0x288280;
pub static ORIGINAL_SKILL_SHOP: OnceLock<unsafe extern "C" fn(custom_skill: usize)> =
    OnceLock::new();

// TODO I would like to make this show a custom message denied message, but for now, just do nothing
pub fn deny_skill_purchasing(custom_skill: usize) {
    if read_data_from_address::<u8>(custom_skill + 0x08) == 0x05 {
        unsafe { replace_single_byte(custom_skill + 0x08, 0x01) }
    }
    if let Some(orig) = ORIGINAL_SKILL_SHOP.get() {
        unsafe {
            orig(custom_skill);
        }
    }
}

pub const GUN_SHOP_ADDR: usize = 0x283d60;
pub static ORIGINAL_GUN_SHOP: OnceLock<unsafe extern "C" fn(custom_gun: usize)> = OnceLock::new();
pub fn deny_gun_upgrade(custom_gun: usize) {
    if read_data_from_address::<u8>(custom_gun + 0x08) == 0x03 {
        unsafe { replace_single_byte(custom_gun + 0x08, 0x01) }
    }
    if let Some(orig) = ORIGINAL_GUN_SHOP.get() {
        unsafe {
            orig(custom_gun);
        }
    }
}

pub const ADD_SHOTGUN_OR_CERBERUS_ADDR: usize = 0x1fcfa0;
pub static ORIGINAL_ADD_SHOTGUN_OR_CERBERUS: OnceLock<
    unsafe extern "C" fn(custom_gun: usize, id: u8) -> bool,
> = OnceLock::new();
// Disabling vanilla behavior of inserting the shotgun/cerberus into the second weapon slot
pub fn deny_cerberus_or_shotgun(_param_1: usize, _id: u8) -> bool {
    false
}

pub const CUSTOMIZE_STYLE_MENU: usize = 0x2b8a10;
pub static ORIGINAL_STYLE_MENU: OnceLock<unsafe extern "C" fn(custom_gun: usize) -> bool> =
    OnceLock::new();
// Control what styles are actually unlocked
pub fn modify_available_styles(data_ptr: usize) -> bool {
    if let Some(mapping) = MAPPING.read().unwrap().as_ref() {
        if mapping.randomize_styles {
            unsafe {
                // Only original 4, quicksilver and doppelganger are controlled by their respective items
                // Trick, Sword, Gun, Royal
                match ARCHIPELAGO_DATA.read() {
                    Ok(data) => {
                        write(
                            (data_ptr + 0x98C6) as *mut [bool; 4],
                            data.get_style_unlocked(),
                        );
                    }
                    Err(err) => {
                        log::error!("Failed to get ArchipelagoData: {}", err)
                    }
                }
            }
        }
    }
    if let Some(orig) = ORIGINAL_STYLE_MENU.get() {
        unsafe {
            return orig(data_ptr);
        }
    }
    false
}

pub const GIVE_STYLE_XP: usize = 0x1fa2c0;
pub static ORIGINAL_GIVE_STYLE_XP: OnceLock<
    unsafe extern "C" fn(ptr: usize, xp_amount: f32) -> f32,
> = OnceLock::new();
// Deny giving style XP
pub fn give_no_xp(param_1: usize, xp_amount: f32) -> f32 {
    if let Some(orig) = ORIGINAL_GIVE_STYLE_XP.get() {
        if MAPPING.read().unwrap().as_ref().unwrap().randomize_styles {
            unsafe { orig(param_1, 0f32) }
        } else {
            unsafe { orig(param_1, xp_amount) }
        }
    } else {
        panic!("Failed to get original give style xp method")
    }
}

pub const SET_NEW_SESSION_DATA: usize = 0x212760; //0x242cc0; // Use current address to have compat with crimson
pub static ORIGINAL_SET_NEW_SESSION_DATA: OnceLock<unsafe extern "C" fn(ptr: usize) -> f32> =
    OnceLock::new();

pub fn set_rando_session_data(ptr: usize) {
    if let Some(orig) = ORIGINAL_SET_NEW_SESSION_DATA.get() {
        unsafe {
            orig(ptr);
        }
    }
    log::debug!("Starting new game, setting appropriate data");
    game_manager::with_session(|s| {
        if s.char != Character::Dante as u8 {
            log::error!(
                "Character is {} not Dante",
                Character::from_repr(s.char as usize).unwrap()
            );
            log::info!("Only Dante is supported at the moment");
            return;
        }
        if let Some(mapping) = MAPPING.read().unwrap().as_ref() {
            // Set initial style if relevant
            if mapping.randomize_styles {
                if let Some(index) = ARCHIPELAGO_DATA
                    .read()
                    .unwrap()
                    .get_style_unlocked()
                    .iter()
                    .position(|&x| x)
                {
                    let style = Style::from_repr(index).unwrap();
                    s.style = style.get_internal_order() as u32;
                }
            }
            // Set starter weapons
            s.weapons[0] = get_weapon_id(mapping.start_melee.as_str());
            s.weapons[1] = 0xFF;
            s.weapons[2] = get_weapon_id(mapping.start_gun.as_str());
            s.weapons[3] = 0xFF;
            // Disable buying blue/purple orbs
            s.items[7] = 6;
            s.items[8] = 7;
            // Unlock DT off the bat
            s.unlocked_dt = true;
            /* Should see if I can change unlocked files? Or unlock them all.
            Game seemed to just auto unlock them though when the weapon is used
            Overall, not too important */
            // This is also where I'd want to set what missions are available, but I haven't figured that out yet
            // 29A5E8
            // 0x45FECCA
        }
    })
    .unwrap();
}
