use crate::archipelago::TX_DEATHLINK;
use crate::constants::*;
use crate::constants::{ItemEntry, Style};
use crate::data::game_structs::{CharacterData, GameData, MissionData, SessionData, TotalRankings};
use crate::data::generated_locations;
use crate::game_manager::{
    ARCHIPELAGO_DATA, get_difficulty, get_mission, get_room, set_item, set_loc_chk_flg,
    set_weapons_in_inv,
};
use crate::hooks::{check_handler, save_handler, store_hook};
use crate::location_handler::in_key_item_room;
use crate::mapping::{Goal, MAPPING, Mapping, run_scouts_for_room};
use crate::tracker::{StyleLevels, initial_connection_updates, send_room_transition};
use crate::ui::text_handler;
use crate::ui::text_handler::LAST_OBTAINED_ID;
use crate::utilities::{DMC3_ADDRESS, read_data_from_address};
use crate::{AP_CORE, archipelago, create_hook, game_manager, skill_manager, utilities};
use archipelago_rs::CreateAsHint;
use bitflags::bitflags;
use minhook::{MH_STATUS, MinHook};
use randomizer_utilities::archipelago_utilities::DeathLinkData;
use randomizer_utilities::item_sync::CURRENT_INDEX;
use randomizer_utilities::replace_single_byte;
use std::arch::asm;
use std::cmp::min;
use std::ptr::write;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, OnceLock};
use std::{ptr, slice};

// 23d680 - Pause menu event? Hook in here to do rendering
pub(crate) unsafe fn create_hooks() -> Result<(), MH_STATUS> {
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
            CUSTOMIZE_STYLE_MENU,
            modify_available_styles,
            ORIGINAL_STYLE_MENU,
            "Hide styles that aren't available"
        );
        create_hook!(
            GIVE_STYLE_XP,
            give_xp_hook,
            ORIGINAL_GIVE_STYLE_XP,
            "Don't give style XP"
        );
        create_hook!(
            SET_NEW_SESSION_DATA,
            set_rando_session_data,
            ORIGINAL_SET_NEW_SESSION_DATA,
            "Set session data for new game"
        );
        create_hook!(
            SELECT_MISSION_BUTTON,
            rewrite_mission_order,
            ORIGINAL_SELECT_MISSION_BUTTON,
            "Edit which mission is loaded on selection"
        );
        create_hook!(
            RESULT_SCREEN_BUTTON_ADDR,
            set_actual_mission,
            ORIGINAL_RESULT_SCREEN_BUTTON,
            "Change which mission is loaded when selecting next"
        );
        create_hook!(
            MISSION_SELECT_SCREEN_CONSTRUCTOR_ADDR,
            mission_select_screen_loaded,
            ORIGINAL_MISSION_SELECT_SCREEN_CONSTRUCTOR,
            "Mission Select Constructor"
        );
        text_handler::setup_text_hooks()?;
        save_handler::setup_save_hooks()?;
        store_hook::create_hooks()?;
    }
    Ok(())
}

static HOOK_ADDRESSES: LazyLock<Vec<usize>> = LazyLock::new(|| {
    const ADDRESSES: [usize; 24] = [
        // Check handling
        check_handler::ITEM_HANDLE_PICKUP_ADDR,
        check_handler::ITEM_PICKED_UP_ADDR,
        check_handler::RESULT_CALC_ADDR,
        check_handler::PURCHASE_ITEM_ADDR,
        // Misc
        ITEM_SPAWNS_ADDR, // Handles replacing item spawns.
        EDIT_EVENT_HOOK_ADDR,
        LOAD_NEW_ROOM_ADDR,
        EQUIPMENT_SCREEN_ADDR,
        SETUP_PLAYER_DATA_ADDR,
        DAMAGE_CALC_ADDR,
        ADJUDICATOR_DATA_ADDR,
        CUSTOMIZE_STYLE_MENU,
        GIVE_STYLE_XP,
        SET_NEW_SESSION_DATA,
        SELECT_MISSION_BUTTON,
        RESULT_SCREEN_BUTTON_ADDR,
        MISSION_SELECT_SCREEN_CONSTRUCTOR_ADDR,
        // Save handler
        save_handler::LOAD_GAME_ADDR,
        save_handler::SAVE_GAME_ADDR,
        save_handler::LOAD_SESSION_DATA,
        save_handler::SAVE_SESSION_DATA,
        // Text Handler
        text_handler::DISPLAY_ITEM_GET_ADDR,
        text_handler::DISPLAY_ITEM_GET_DESTRUCTOR_ADDR,
        text_handler::SETUP_ITEM_GET_SCREEN_ADDR,
    ];
    let mut addrs = ADDRESSES.to_vec();
    addrs.extend(store_hook::HOOK_ADDRESSES.iter());
    addrs
});

pub(crate) fn enable_hooks() {
    HOOK_ADDRESSES.iter().for_each(|addr| unsafe {
        if let Err(err) = MinHook::enable_hook((*DMC3_ADDRESS + addr) as *mut _) {
            log::error!("Failed to enable {:#X} hooks: {:?}", addr, err);
        }
    })
}

/// Disable hooks, used for disconnecting
pub fn disable_hooks() -> Result<(), MH_STATUS> {
    let base_address = *DMC3_ADDRESS;
    HOOK_ADDRESSES.iter().for_each(|addr| unsafe {
        if let Err(err) = MinHook::disable_hook((base_address + *addr) as *mut _) {
            log::error!("Failed to disable {:#X} hooks: {:?}", addr, err);
        }
    });
    Ok(())
}

pub const EDIT_EVENT_HOOK_ADDR: usize = 0x1a9bc0;
pub static ORIGINAL_EDIT_EVENT: OnceLock<
    unsafe extern "C" fn(param_1: usize, param_2: i32, param_3: usize),
> = OnceLock::new();
pub fn edit_event_drop(param_1: usize, param_2: i32, param_3: usize) {
    let (mapping, mission_event_tables, checked_locations) =
        if let (Ok(mapping), Some(mission_event_tables), Ok(client)) = (
            MAPPING.read(),
            EVENT_TABLES.get(&get_mission()),
            AP_CORE.get().unwrap().as_ref().lock(),
        ) && mapping.is_some()
        {
            (mapping, mission_event_tables, client)
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
                            if checked_locations
                                .connection
                                .client()
                                .unwrap()
                                .checked_locations()
                                .any(|loc| loc.name() == event_table.location)
                            {
                                log::debug!("Event loc checked: {}", &event_table.location);
                                match event.event_type {
                                    // If the location has already been checked use DUMMY_ID as a dummy item.
                                    EventCode::Give => replace_single_byte(
                                        event_table_addr + event.offset,
                                        *DUMMY_ID as u8,
                                    ),
                                    EventCode::Check => replace_single_byte(
                                        event_table_addr + event.offset,
                                        *DUMMY_ID as u8,
                                    ),
                                    EventCode::End => replace_single_byte(
                                        event_table_addr + event.offset,
                                        *DUMMY_ID as u8,
                                    ),
                                }
                            } else {
                                log::debug!("Event loc not checked: {}", &event_table.location);
                                match event.event_type {
                                    // Location has not been checked off!
                                    EventCode::Give => {}
                                    EventCode::Check => {
                                        log::debug!("Replaced check at {:#X}", &event.offset);
                                        replace_single_byte(
                                            event_table_addr + event.offset,
                                            *DUMMY_ID as u8,
                                        )
                                    }
                                    EventCode::End => {
                                        log::debug!("Replaced end at {:#X}", &event.offset);
                                        replace_single_byte(
                                            event_table_addr + event.offset,
                                            *DUMMY_ID as u8,
                                        )
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
    randomizer_utilities::modify_protected_memory(
        || {
            const LENGTH: usize = 16;
            unsafe {
                let table = slice::from_raw_parts_mut(table_address as *mut u8, LENGTH);
                table.fill(0x01u8); // 0 = orbs, 1 = items, 2 = bad
            }
        },
        table_address as *mut [u8; 16],
    )
    .unwrap();
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
    const LOG_ADJU_DATA: bool = false;
    const RANKING_OFFSET: usize = 0x04;
    const WEAPON_OFFSET: usize = 0x06;
    for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
        // Run through all locations
        if entry.adjudicator && entry.room_number == get_room() {
            if LOG_ADJU_DATA {
                log::debug!("Adjudicator found at location {}", &location_name);
                log::debug!(
                    "Rank Needed: {}",
                    Rank::from_repr(
                        read_data_from_address::<u8>(adjudicator_data + RANKING_OFFSET) as usize
                            - 1
                    )
                    .unwrap_or_else(|| panic!(
                        "Unable to get rank from adjudicator: {}",
                        read_data_from_address::<u8>(adjudicator_data + RANKING_OFFSET)
                    ))
                );
                log::debug!(
                    "Melee: {}",
                    read_data_from_address::<u8>(adjudicator_data + WEAPON_OFFSET)
                );
            }

            if let Some(mappings) = MAPPING.read().unwrap().as_ref()
                && let Some(adjudicator_map) = &mappings.adjudicators
                && let Some(data) = adjudicator_map.get(*location_name)
            {
                //log::debug!("New adjudicator data will be {:?}", data);
                unsafe {
                    replace_single_byte(adjudicator_data + RANKING_OFFSET, data.ranking);
                    replace_single_byte(
                        adjudicator_data + WEAPON_OFFSET,
                        get_unlocked_weapon_id(&data.weapon),
                    );
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
                        const EXTRA_OUTPUT: bool = false;
                        if EXTRA_OUTPUT {
                            log::debug!("Item ID: {} ({:#X})", get_item_name(*item_ref), *item_ref);
                        }
                        for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
                            check_and_replace_item(
                                location_name,
                                entry,
                                room_num,
                                mapping,
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
    let _ = CharacterData::with_read(|c| {
        if c.hp <= 0.0 {
            log::debug!("Dante died!");
            send_deathlink();
        }
    })
    .is_err();
}

fn send_deathlink() {
    TX_DEATHLINK
        .get()
        .unwrap()
        .send(DeathLinkData {
            cause: format!(
                "died in Mission #{} on {}", // TODO Maybe an "against {}" at some point?
                get_mission(),
                get_difficulty()
            ),
        })
        .unwrap();
}

pub const EQUIPMENT_SCREEN_ADDR: usize = 0x28CBD0;
pub static ORIGINAL_EQUIPMENT_SCREEN: OnceLock<unsafe extern "C" fn(cuid_weapon: usize) -> i32> =
    OnceLock::new();
/// Edits the initially selected index when viewing weapons in the status screen
fn edit_initial_index(custom_weapon: usize) -> i32 {
    let base = *DMC3_ADDRESS;
    log::debug!("Editing initial index");
    let starting_index = MissionData::with_read(|m| {
        if read_data_from_address::<u8>(custom_weapon + 0x419E) == 4 {
            // Gun
            for index in 0..5 {
                if m.items[0x1C + index] == 1 {
                    return index;
                }
            }
            if m.items[0x21] == 1 {
                return 4;
            }
        } else {
            // Melee
            for index in 0..=3 {
                if m.items[0x16 + index] == 1 {
                    return index;
                }
            }
            if m.items[0x1A] == 1 {
                // Nevan
                return 3;
            }
            if m.items[0x1B] == 1 {
                // Beowulf
                return 4;
            }
        }
        0
    })
    .unwrap_or_else(|_| {
        log::error!("Unable to calculate starting index for equipment screen");
        0
    });
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
    //unsafe {
    // Skipping if location file has room as 0, that means its either event or not done
    if entry.room_number == 0 {
        return;
    }
    //log::debug!("Room number X: {} Room number memory: {}, Item ID X: {:#X}, Item ID Memory: {:#X}", entry.room_number, room_num, entry.item_id, *item_ref);
    if entry.room_number == room_num && entry.item_id == *item_ref && !entry.adjudicator {
        log::debug!("Seeing if item needs to be dummy");
        if !dummy_replace(location_name, item_addr) {
            // log::info!(
            //     "Replaced item in room {} ({}) with {} {:#X}",
            //     entry.room_number,
            //     location_name,
            //     get_item_name(*item_ref),
            //     *item_addr
            // );
        }
    }
    //}
}

/// Replaces an item with a dummy one in order to not immediately proc end events upon entering the location's room
fn dummy_replace(location_key: &&str, item_addr: *mut i32) -> bool {
    // Get event tables for mission and then each END event
    if let Some(event_tables) = EVENT_TABLES.get(&get_mission()) {
        for event_table in event_tables
            .iter()
            .filter(|table| table.location == *location_key)
        {
            for _ in event_table
                .events
                .iter()
                .filter(|event| event.event_type == EventCode::End)
            {
                if let Ok(core) = AP_CORE.get().unwrap().lock().as_ref() {
                    // Then if location in question is checked, replace the item with a dummy and return true
                    if core
                        .connection
                        .client()
                        .unwrap()
                        .checked_locations()
                        .any(|loc| loc.name() == *location_key)
                    {
                        unsafe {
                            *item_addr = *DUMMY_ID as i32;
                        }
                        log::info!("Replaced item at {} with dummy item", location_key);
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn set_relevant_key_items() {
    if let Ok(data) = ARCHIPELAGO_DATA.read() {
        SessionData::with_read(|s| {
            match MISSION_ITEM_MAP.get(&(s.mission)) {
                None => {} // No items for the mission
                Some(item_list) => {
                    for item in item_list.iter() {
                        if data.items.contains(*item) {
                            let res = game_manager::has_item_by_flags(item);
                            if !res {
                                set_item(item, true, true);
                            }
                            log::debug!(
                                "Item Relevant to mission #{} - {} Flag: {}",
                                s.mission,
                                *item,
                                res
                            );
                        } else {
                            set_item(item, false, true);
                        }
                    }
                }
            }

            if let Ok(core) = AP_CORE.get().unwrap().lock().as_ref() {
                let mut checked_locations = core.connection.client().unwrap().checked_locations();
                // Special case for Ignis Fatuus
                // Needed so the Ignis Fatuus location can be reached even when the actual key item is acquired
                if get_room() == 302
                    && let Some(event_table_addr) = utilities::get_event_address()
                {
                    if checked_locations.any(|loc| loc.name() == "Mission #8 - Ignis Fatuus") {
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

                if let Ok(loc) = in_key_item_room() {
                    log::debug!("In key room: {}", loc);
                    if !checked_locations.any(|location| location.name() == loc) {
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
    match AP_CORE.get().unwrap().lock() {
        Ok(mut core) => {
            let client = core.connection.client_mut().unwrap();
            run_scouts_for_room(client, CreateAsHint::No);
            if let Err(e) = send_room_transition(client, false) {
                log::error!("Failed to send room transition: {}", e);
            }
        }
        Err(err) => {
            log::error!("Failed to lock AP_CORE: {}", err);
        }
    }
    set_weapons_in_inv(&ARCHIPELAGO_DATA.read().unwrap());
    set_relevant_key_items();
    check_handler::clear_high_roller();
    LAST_OBTAINED_ID.store(0, Ordering::SeqCst); // Should stop random item jumpscares
    if let Some(mapping) = MAPPING.read().unwrap().as_ref()
        && mapping.randomize_skills
    {
        skill_manager::set_skills(&ARCHIPELAGO_DATA.read().unwrap());
    }
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
        if mapping.randomize_gun_levels {
            game_manager::set_gun_levels(&ARCHIPELAGO_DATA.read().unwrap());
        }
        if mapping.randomize_skills {
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

/// Set the modified modes back to 1 from 2
pub(crate) fn restore_mode_table() {
    let table_address = ITEM_MODE_TABLE + *DMC3_ADDRESS;
    const LENGTH: usize = 16;
    randomizer_utilities::modify_protected_memory(
        || {
            unsafe {
                let table = slice::from_raw_parts_mut(table_address as *mut u8, LENGTH);
                table.fill(0x02u8); // 0 = orbs, 1 = items, 2 = bad
            }
        },
        table_address as *mut [u8; LENGTH],
    )
    .unwrap();
}

pub const CUSTOMIZE_STYLE_MENU: usize = 0x2b8a10;
pub static ORIGINAL_STYLE_MENU: OnceLock<unsafe extern "C" fn(custom_gun: usize) -> bool> =
    OnceLock::new();
// Control what styles are actually unlocked
pub fn modify_available_styles(data_ptr: usize) -> bool {
    if let Some(mapping) = MAPPING.read().unwrap().as_ref()
        && mapping.randomize_styles
    {
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
pub fn give_xp_hook(param_1: usize, xp_amount: f32) -> f32 {
    if let Some(orig) = ORIGINAL_GIVE_STYLE_XP.get() {
        // Get original style level

        let current_style_level =
            CharacterData::with_read(|c| c.style_level).unwrap_or_else(|_| {
                log::error!("Unable to get current style level");
                0
            });
        let res = if MAPPING.read().unwrap().as_ref().unwrap().randomize_styles {
            unsafe { orig(param_1, 0f32) }
        } else {
            // If Styles are items in world, do not give XP
            unsafe { orig(param_1, xp_amount) }
        };

        CharacterData::with_read(|c| {
            let new_style_level = c.style_level;
            if new_style_level > current_style_level {
                // TODO I don't know if I like this
                if let Ok(mut core) = AP_CORE.get().unwrap().as_ref().lock()
                    && let Some(client) = core.connection.client_mut()
                    && let Err(e) = StyleLevels::update(
                        Style::INTERNAL_ORDER[c.style as usize],
                        new_style_level,
                        client,
                    )
                {
                    log::error!("Failed to update StyleLevels: {}", e);
                }
            }
        })
        .unwrap_or_else(|_| {
            log::error!("Unable to get current style");
        });

        res
    } else {
        panic!("Failed to get original give style xp method")
    }
}

pub const SET_NEW_SESSION_DATA: usize = 0x212760; //0x242cc0; // Use current address to have compat with crimson
pub static ORIGINAL_SET_NEW_SESSION_DATA: OnceLock<unsafe extern "C" fn(ptr: usize) -> f32> =
    OnceLock::new();

bitflags! {
    #[derive(Debug)]
    struct DifficultyUnlockFlags: u8 {
        const Easy = 0b00000001;
        const Normal = 0b00000000; // Always unlocked, does not have a flag as far as I know
        const Hard = 0b00000010;
        const VeryHard = 0b00000100;
        const DanteMustDie = 0b00001000;
        const HeavenOrHell = 0b00010000;
    }
}

impl DifficultyUnlockFlags {
    fn create_final_flag(difficulties: &Vec<Difficulty>) -> DifficultyUnlockFlags {
        let mut res = DifficultyUnlockFlags::empty();
        for difficulty in difficulties {
            res = res.union(match difficulty {
                Difficulty::Easy => DifficultyUnlockFlags::Easy,
                Difficulty::Normal => DifficultyUnlockFlags::Normal,
                Difficulty::Hard => DifficultyUnlockFlags::Hard,
                Difficulty::VeryHard => DifficultyUnlockFlags::VeryHard,
                Difficulty::DanteMustDie => DifficultyUnlockFlags::DanteMustDie,
                Difficulty::HeavenOrHell => DifficultyUnlockFlags::HeavenOrHell,
            });
        }
        res
    }
}

bitflags! {
    #[derive(Debug)]
    struct UnlockFlags: u8 {
        const DMC1DanteCostume = 0b10000000;
        const ShirtlessDanteCostume = 0b01000000;
        const Unk3 = 0b00100000;
        const Vergil = 0b00010000;
        const BloodyPalace = 0b00001000;
        const Unk6 = 0b00000100;
        const Gallery = 0b00000010;
        const MissionSelect = 0b00000001;
    }
}

impl UnlockFlags {
    fn create_final_flag() -> UnlockFlags {
        let mut res = UnlockFlags::empty();
        res = res.union(UnlockFlags::DMC1DanteCostume);
        res = res.union(UnlockFlags::ShirtlessDanteCostume);
        //res = res.union(UnlockFlags::Unk3);
        //res = res.union(UnlockFlags::Vergil);
        res = res.union(UnlockFlags::BloodyPalace);
        //res = res.union(UnlockFlags::Unk6);
        res = res.union(UnlockFlags::Gallery);
        res = res.union(UnlockFlags::MissionSelect);
        res
    }
}

bitflags! {
    #[derive(Debug)]
    struct CostumeFlags: u8 {
        const SuperCorruptVergil = 0b10000000;
        const CorruptVergil = 0b01000000;
        const SuperVergil = 0b00100000;
        const CoatlessVergil = 0b00010000;
        const SuperSparda = 0b00001000;
        const Sparda = 0b00000100;
        const SuperDante = 0b00000010;
        const CoatlessDMC1Dante = 0b00000001;
    }
}

impl CostumeFlags {
    fn create_final_flag() -> CostumeFlags {
        let mut res = CostumeFlags::empty();
        //res = res.union(CostumeFlags::SuperCorruptVergil);
        res = res.union(CostumeFlags::CorruptVergil);
        //res = res.union(CostumeFlags::SuperVergil);
        res = res.union(CostumeFlags::CoatlessVergil);
        //res = res.union(CostumeFlags::SuperSparda);
        res = res.union(CostumeFlags::Sparda);
        //res = res.union(CostumeFlags::SuperDante);
        res = res.union(CostumeFlags::CoatlessDMC1Dante);
        res
    }
}

// Could do gallery here, but I see no reason to

pub fn set_rando_session_data(ptr: usize) {
    if let Some(orig) = ORIGINAL_SET_NEW_SESSION_DATA.get() {
        unsafe {
            orig(ptr);
        }
    }
    log::debug!("Starting new game, setting appropriate data");
    SessionData::with_mut(|s| {
        if s.char != Character::Dante as u8 {
            log::error!(
                "Character is {} not Dante",
                Character::from_repr(s.char as usize).unwrap()
            );
            log::info!("Only Dante is supported at the moment");
            return;
        }
        if let Some(mapping) = MAPPING.read().unwrap().as_ref() {
            // Unlock difficulties, costumes and modes
            unsafe {
                let difficulty_flags = DifficultyUnlockFlags::create_final_flag(
                    &mapping.initially_unlocked_difficulties,
                );
                let unlock_flags = UnlockFlags::create_final_flag();
                let costume_flags = CostumeFlags::create_final_flag();
                replace_single_byte(*DMC3_ADDRESS + 0x564594, difficulty_flags.bits());
                replace_single_byte(*DMC3_ADDRESS + 0x564595, unlock_flags.bits());
                replace_single_byte(*DMC3_ADDRESS + 0x564596, costume_flags.bits());
            }
            // Set initial style if relevant
            if mapping.randomize_styles
                && let Some(index) = ARCHIPELAGO_DATA
                    .read()
                    .unwrap()
                    .get_style_unlocked()
                    .iter()
                    .position(|&x| x)
            {
                let style = Style::from_repr(index).unwrap();
                s.style = style.get_internal_order_index() as u32;
            }

            // Set starter weapons
            s.weapons[0] = 0xFF; //mapping.start_melee;
            //s.weapons[1] = mapping.start_second_melee;
            s.weapons[2] = 0xFF; //mapping.start_gun;
            //s.weapons[3] = mapping.start_second_gun;

            // Unlock DT off the bat
            s.unlocked_dt = true;
            /* Should see if I can change unlocked files? Or unlock them all.
            Game seemed to just auto unlock them though when the weapon is used
            Overall, not too important */
            s.red_orbs = 0;
            #[cfg(debug_assertions)]
            {
                // Give max red orbs if we are using a debug build
                //s.red_orbs = i32::MAX;
            }
            // 29A5E8
            // 0x45FECCA
            if mapping.goal == Goal::RandomOrder {
                s.mission = mapping.mission_order.as_ref().unwrap()[0] as u32;
                s.other_mission = mapping.mission_order.as_ref().unwrap()[0] as u32;
            }

            // If orb checks are disabled, then set the purchase count to max for both to prevent purchasing
            if !mapping.shop_orb_checks {
                s.items[7] = 6;
                s.items[8] = 7;
            }
        }
    })
    .unwrap();
    skill_manager::set_skills(&ARCHIPELAGO_DATA.read().unwrap());
    set_weapons_in_inv(&ARCHIPELAGO_DATA.read().unwrap());
    match AP_CORE.get().unwrap().lock() {
        Ok(mut core) => {
            CURRENT_INDEX.store(0, Ordering::SeqCst);
            let client = core.connection.client_mut().unwrap();
            if let Err(e) = archipelago::handle_received_items_packet(0, client) {
                log::error!("Failed to handle received items: {:?}", e);
            }
            if let Err(e) = initial_connection_updates(client) {
                log::error!("Failed to set datastorage values for tracker: {:?}", e);
            }
        }
        Err(err) => {
            log::error!("Error locking core: {}", err);
        }
    }
}

pub const SELECT_MISSION_BUTTON: usize = 0x29a7b0;
pub static ORIGINAL_SELECT_MISSION_BUTTON: OnceLock<unsafe extern "C" fn(ptr: usize)> =
    OnceLock::new();

pub fn rewrite_mission_order(ptr: usize) {
    // Running the original method replaces this val with something else, 0 won't be seen again
    let val = read_data_from_address::<u8>(ptr + 0x08);
    if let Some(orig) = ORIGINAL_SELECT_MISSION_BUTTON.get() {
        unsafe {
            orig(ptr);
        }
    }
    if let Some(mapping) = MAPPING.read().unwrap().as_ref() {
        match val {
            0 => {
                if mapping.goal != Goal::Standard {
                    SessionData::with_read(|s| {
                        // Sets the mission selected on the select screen to be the correct one
                        unsafe {
                            replace_single_byte(
                                ptr + 0x628A + s.difficulty as usize,
                                1 + mapping.get_index_for_mission(s.mission) as u8,
                            );
                        }
                    })
                    .unwrap();
                    for difficulty in 0..6 {
                        let max_mission = calculate_max_mission(
                            mapping,
                            Difficulty::from_repr(difficulty).unwrap(),
                        );
                        log::debug!(
                            "Max mission for {} is {}",
                            Difficulty::from_repr(difficulty).unwrap(),
                            max_mission
                        );
                        unsafe {
                            replace_single_byte(ptr + 0x6290 + difficulty, max_mission);
                        }
                    }
                }
            }
            6 => {
                if mapping.goal == Goal::RandomOrder {
                    SessionData::with_mut(|s| {
                        log::debug!("Original Mission was: {}", s.mission);
                        log::debug!("Original O Mission was: {}", s.other_mission);
                        if let Some(mission_order) = &mapping.mission_order {
                            let mission_idx = s.mission as usize;
                            s.mission = mission_order[mission_idx - 1] as u32;
                            s.other_mission = mission_order[mission_idx - 1] as u32;
                        }
                    })
                    .expect("Unable to edit session data");
                }
            }
            1_u8..=5_u8 | 7_u8..=u8::MAX => {}
        }
    }
}

pub fn calculate_max_mission(mapping: &Mapping, difficulty: Difficulty) -> u8 {
    const NOT_COMPLETED: u8 = 0xFF;
    TotalRankings::with_read(|r| {
        match mapping.goal {
            Goal::RandomOrder => {
                // Check the rankings, this is how we know what missions are available
                let rankings = r.get_ranking_for_difficulty(difficulty);
                let mut max_idx = 1;
                // For each mission, see if it's completed
                // If it is, then increment max_idx
                mapping
                    .mission_order
                    .as_ref()
                    .unwrap()
                    .iter()
                    .for_each(|val| {
                        if rankings[(*val - 1) as usize] != NOT_COMPLETED {
                            max_idx += 1;
                        }
                    });
                max_idx
            }
            Goal::Standard => {
                // Check the rankings, this is how we know what missions are available
                // TODO I could probably reduce some code dupe here.
                let rankings = r.get_ranking_for_difficulty(difficulty);
                let mut max_idx = 1;
                // For each mission, see if it's completed
                // If it is, then increment max_idx
                static DEFAULT_ORDER: [u8; 20] = [
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
                ];
                DEFAULT_ORDER.iter().for_each(|val| {
                    if rankings[(*val - 1) as usize] != NOT_COMPLETED {
                        max_idx += 1;
                    }
                });
                max_idx
            }
            Goal::All => 20,
        }
    })
    .unwrap()
}

pub const MISSION_SELECT_SCREEN_CONSTRUCTOR_ADDR: usize = 0x2999a0;
pub static ORIGINAL_MISSION_SELECT_SCREEN_CONSTRUCTOR: OnceLock<
    unsafe extern "C" fn(usize) -> usize,
> = OnceLock::new();

fn mission_select_screen_loaded(mis_select: usize) -> usize {
    let res = if let Some(orig) = ORIGINAL_MISSION_SELECT_SCREEN_CONSTRUCTOR.get() {
        unsafe { orig(mis_select) }
    } else {
        panic!("Failed to find original constructor for mission select screen");
    };
    match AP_CORE.get().unwrap().lock() {
        Ok(mut core) => {
            let client = core.connection.client_mut().unwrap();
            if let Err(e) = send_room_transition(client, true) {
                log::error!("Failed to send room transition: {}", e);
            }
        }
        Err(err) => {
            log::error!("Failed to lock AP_CORE: {}", err);
        }
    }
    res
}

pub const RESULT_SCREEN_BUTTON_ADDR: usize = 0x241fe0;
pub static ORIGINAL_RESULT_SCREEN_BUTTON: OnceLock<
    unsafe extern "C" fn(usize, usize, usize, usize) -> i32,
> = OnceLock::new();

fn set_actual_mission(cscene_result: usize, param_1: usize, param_2: usize, param_3: usize) -> i32 {
    let val = read_data_from_address::<i32>(cscene_result + 0x14);
    let current_mission = get_mission();
    let res = if let Some(orig) = ORIGINAL_RESULT_SCREEN_BUTTON.get() {
        unsafe { orig(cscene_result, param_1, param_2, param_3) }
    } else {
        panic!("Failed to find original method for result screen");
    };
    if let Some(mapping) = MAPPING.read().unwrap().as_ref()
        && mapping.goal == Goal::RandomOrder
    {
        match val {
            0x12 => {
                SessionData::with_mut(|s| {
                    s.mission = mapping.mission_order.as_ref().unwrap()
                        [min(mapping.get_index_for_mission(current_mission) + 1, 19)]
                        as u32;
                })
                .unwrap();
            }
            _ => {}
        }
    }
    res
}
