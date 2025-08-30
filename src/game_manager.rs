use crate::constants::{
    get_items_by_category, get_weapon_id, ItemCategory, BASE_HP, GUN_NAMES, ITEM_ID_MAP, ITEM_OFFSET_MAP, MAX_HP,
    MAX_MAGIC, MELEE_NAMES, ONE_ORB,
};
use crate::item_sync;
use crate::ui::ui::CHECKLIST;
use crate::utilities::{
    get_inv_address, read_data_from_address, replace_single_byte, DMC3_ADDRESS,
};
use std::collections::HashMap;
use std::ptr::{read_unaligned, write_unaligned};
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, RwLockReadGuard};

pub(crate) const GAME_SESSION_DATA: usize = 0xC8F250;

#[repr(C)]
pub struct SessionData {
    pub(crate) mission: u32,
    other_mission: u32, // Don't know what this does, copies from mission after a second
    pub(crate) room: i32, // Should be right?
    pub(crate) difficulty: u32,
    hoh: bool,
    _unknown2: u8,
    tutorial: bool,
    gold_orb_mode: bool,
    char: u8,
    _unknown3: [u8; 7],
    bloody_palace: bool,
    _unknown4: [u8; 15],
    red_orbs: u32,
    items: [u8; 20],
    unknown5: [u8; 2],
    unlocks: [bool; 14],
    unknown6: [u8; 48],
    weapons: [u8; 8],
    unknown7: [u8; 20],
    ranged_weapon_levels: [u32; 5],
    unknown8: [u8; 20],
    melee_index: u32,
    gun_index: u32,
    costume: u8,
    unlocked_dt: bool,
    unknown9: [u8; 2],
    pub max_hp: f32,
    pub max_magic: f32,
    style: u32,
    style_levels: [u32; 6],
    style_xp: [f32; 6],
    expertise: [u8; 8],
}

/// Error type for session access
#[derive(Debug)]
pub enum SessionError {
    NotUsable, // If a save slot has not been loaded for whatever reason
}

static SESSION_PTR: LazyLock<usize> =
    LazyLock::new(|| *DMC3_ADDRESS + GAME_SESSION_DATA);

pub fn with_session_read<F, R>(f: F) -> Result<R, SessionError>
where
    F: FnOnce(&SessionData) -> R,
{
    let addr = *SESSION_PTR;
    unsafe {
        let s = &*(addr as *const SessionData);
        if !session_is_valid(s) {
            return Err(SessionError::NotUsable);
        }
        Ok(f(s))
    }
}

pub fn with_session<F, R>(f: F) -> Result<R, SessionError>
where
    F: FnOnce(&mut SessionData) -> R,
{
    let addr = *SESSION_PTR;
    unsafe {
        let s = &mut *(addr as *mut SessionData);
        if !session_is_valid(s) {
            return Err(SessionError::NotUsable);
        }
        Ok(f(s))
    }
}

fn session_is_valid(_s: &SessionData) -> bool {
    true
}

/// Get current mission
pub fn get_mission() -> u32 {
    with_session_read(|s| s.mission).unwrap()
}

/// Get current room
pub fn get_room() -> i32 {
    with_session_read(|s| s.room).unwrap()
}

const CHARACTER_DATA: usize = 0xC90E30;
pub(crate) const ACTIVE_CHAR_DATA: usize = 0xCF2548;

pub(crate) fn give_magic(magic_val: f32) {
    let base = *DMC3_ADDRESS;
    unsafe {
        write_unaligned(
            (base + GAME_SESSION_DATA + 0xD8) as *mut f32,
            read_unaligned((base + GAME_SESSION_DATA + 0xD8) as *mut f32) + magic_val,
        );
        write_unaligned(
            (base + CHARACTER_DATA + 0x16C + 0x6C) as *mut f32,
            read_unaligned((base + CHARACTER_DATA + 0x16C + 0x6C) as *mut f32) + magic_val,
        ); // Magic
        write_unaligned(
            (base + CHARACTER_DATA + 0x16C + 0x70) as *mut f32,
            read_unaligned((base + CHARACTER_DATA + 0x16C + 0x70) as *mut f32) + magic_val,
        ); // Max magic
        let char_data_ptr: usize =
            read_data_from_address(*DMC3_ADDRESS + ACTIVE_CHAR_DATA);
        if char_data_ptr != 0 {
            write_unaligned(
                (char_data_ptr + 0x3EB8) as *mut f32,
                read_unaligned((char_data_ptr + 0x3EB8) as *mut f32) + magic_val,
            ); // Magic char
            write_unaligned(
                (char_data_ptr + 0x3EBC) as *mut f32,
                read_unaligned((char_data_ptr + 0x3EBC) as *mut f32) + magic_val,
            ); // Max magic char
        }
    }
}

pub(crate) fn give_hp(life_value: f32) {
    let base = *DMC3_ADDRESS;
    unsafe {
        log::debug!("Normal data");
        write_unaligned(
            (base + CHARACTER_DATA + 0x16C + 0x64) as *mut f32,
            read_unaligned((base + CHARACTER_DATA + 0x16C + 0x64) as *mut f32) + life_value,
        ); // Life
        write_unaligned(
            (base + CHARACTER_DATA + 0x16C + 0x68) as *mut f32,
            read_unaligned((base + CHARACTER_DATA + 0x16C + 0x68) as *mut f32) + life_value,
        ); // Max life
        let char_data_ptr: usize =
            read_data_from_address(*DMC3_ADDRESS + ACTIVE_CHAR_DATA);
        if char_data_ptr != 0 {
            write_unaligned(
                (char_data_ptr + 0x411C) as *mut f32,
                read_unaligned((char_data_ptr + 0x411C) as *mut f32) + life_value,
            ); // Life char
            write_unaligned(
                (char_data_ptr + 0x40EC) as *mut f32,
                read_unaligned((char_data_ptr + 0x40EC) as *mut f32) + life_value,
            ); // Max Life char
        }
    }
}

/// Use for weapons/key items
pub(crate) fn set_item(item_name: &str, has_item: bool, set_flag: bool) {
    if get_inv_address().is_none() {
        return;
    }
    unsafe {
        replace_single_byte(
            get_inv_address().unwrap() + ITEM_OFFSET_MAP.get(item_name).unwrap().clone() as usize,
            has_item as u8,
        )
    };
    if set_flag {
        set_loc_chk_flg(item_name, has_item);
    }
}

const LOCATION_FLAGS: usize = 0xc90e28;
pub fn set_loc_chk_flg(item_name: &str, set_flag: bool) {
    let ptr: usize = read_data_from_address(*DMC3_ADDRESS + LOCATION_FLAGS);
    let item_id: i32 = *ITEM_ID_MAP.get(item_name).unwrap() as i32;
    let loc_chk_flags = read_data_from_address::<usize>(ptr + 0x30);

    let item_flag: usize = (item_id + (item_id >> 0x1F & 0x7) >> 3) as usize;
    let mask: u8 = 1 << (item_id & 7);

    unsafe {
        for base in [0x7DAusize, 0x7E2usize] {
            let addr = loc_chk_flags + item_flag + base;
            let val = read_data_from_address::<u8>(addr);
            if set_flag {
                replace_single_byte(addr, val | mask);
            } else {
                replace_single_byte(addr, val & !mask);
            }
        }
    }
}

pub fn has_item_by_flags(item_name: &str) -> bool {
    let ptr: usize = read_data_from_address(*DMC3_ADDRESS + LOCATION_FLAGS);
    let item_id: i32 = *ITEM_ID_MAP.get(item_name).unwrap() as i32;
    let loc_chk_flags = read_data_from_address::<usize>(ptr + 0x30);
    let item_flag: usize = (item_id + (item_id >> 0x1F & 0x7) >> 3) as usize;
    let mask: u8 = 1 << (item_id & 7);

    for base in [0x7DAusize, 0x7E2usize] {
        let addr = loc_chk_flags + item_flag + base;
        let byte = read_data_from_address::<u8>(addr);
        if (byte & mask) == 0 {
            return false;
        }
    }
    true
}

pub fn set_max_hp_and_magic() {
    with_session(|s| {
        log::debug!(
            "Modifying player attributes- Original HP: {}, Magic: {}",
            s.max_hp,
            s.max_magic
        );
        s.max_hp = f32::min(
            BASE_HP + (item_sync::BLUE_ORBS_OBTAINED.load(Ordering::SeqCst) as f32 * ONE_ORB),
            MAX_HP,
        );
        log::debug!("New HP is: {}", s.max_hp);
        s.max_magic = f32::min(
            item_sync::PURPLE_ORBS_OBTAINED.load(Ordering::SeqCst) as f32 * ONE_ORB,
            MAX_MAGIC,
        );
        log::debug!("New Magic is: {}", s.max_magic);
    })
    .unwrap();
}

pub(crate) fn kill_dante() {
    let char_data_ptr: usize =
        read_data_from_address(*DMC3_ADDRESS + ACTIVE_CHAR_DATA);
    unsafe {
        write_unaligned((char_data_ptr + 0x411C) as *mut f32, 0.0);
    }
}

pub fn set_session_weapons() {
    let checklist: RwLockReadGuard<HashMap<String, bool>> =
        CHECKLIST.get().unwrap().read().unwrap();
    with_session(|s| {
        for weapon in get_items_by_category(ItemCategory::Weapon) {
            if *checklist.get(weapon).unwrap_or(&false) {
                let weapon_id = get_weapon_id(weapon);
                if MELEE_NAMES.contains(&weapon) {
                    if s.weapons[0] != weapon_id && s.weapons[1] == 0xFF {
                        log::debug!("Inserting {} into second melee slot", weapon);
                        s.weapons[1] = weapon_id;
                    }
                }
                if GUN_NAMES.contains(&weapon) {
                    if s.weapons[2] != weapon_id && s.weapons[3] == 0xFF {
                        log::debug!("Inserting {} into second gun slot", weapon);
                        s.weapons[3] = weapon_id;
                    }
                }
            }
        }
    })
    .unwrap();
}
//const WEAPON_SLOT: usize = 0x045FF2D8;
pub(crate) fn set_weapons_in_inv() {
    let checklist: RwLockReadGuard<HashMap<String, bool>> =
        CHECKLIST.get().unwrap().read().unwrap();
    let mut flag;
    for weapon in get_items_by_category(ItemCategory::Weapon) {
        if *checklist.get(weapon).unwrap_or(&false) {
            flag = true;
            log::debug!("Adding weapon/style to inventory {}", weapon);
        } else {
            flag = false;
        }
        set_item(weapon, flag, true);
    }
}
