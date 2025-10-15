use crate::constants::{
    get_items_by_category, get_weapon_id, Difficulty, ItemCategory, BASE_HP, GUN_NAMES, ITEM_ID_MAP, ITEM_OFFSET_MAP,
    MAX_HP, MAX_MAGIC, MELEE_NAMES, ONE_ORB,
};
use crate::hook::ORIGINAL_GIVE_STYLE_XP;
use crate::ui::ui::CHECKLIST;
use crate::utilities::{
    get_inv_address, read_data_from_address, replace_single_byte, DMC3_ADDRESS,
};
use std::collections::HashMap;
use std::ptr::{read_unaligned, write_unaligned};
use std::sync::{LazyLock, RwLock, RwLockReadGuard};
use crate::mapping::MAPPING;

pub(crate) const GAME_SESSION_DATA: usize = 0xC8F250;

#[derive(Debug, Default)]
pub(crate) struct ArchipelagoData {
    pub(crate) blue_orbs: i32,
    pub(crate) purple_orbs: i32,
    pub(crate) dt_unlocked: bool,
    gun_levels: [u32; 5],
    style_levels: [i32; 4],
    pub(crate) stinger_level: u8,
    pub(crate) jet_stream_level: u8,
    pub(crate) reverb_level: u8,
}

#[derive(Copy, Clone, strum_macros::Display, strum_macros::FromRepr)]
pub(crate) enum Style {
    Trickster = 0,
    Swordmaster = 1,
    Gunslinger = 2,
    Royalguard = 3,
}

impl Style {
    pub fn index(&self) -> usize {
        *self as usize
    }

    pub fn get_internal_order(&self) -> usize {
        match &self {
            Style::Trickster => 2,
            Style::Swordmaster => 0,
            Style::Gunslinger => 1,
            Style::Royalguard => 3,
        }
    }
}

pub static ARCHIPELAGO_DATA: LazyLock<RwLock<ArchipelagoData>> =
    LazyLock::new(|| RwLock::new(ArchipelagoData::default()));

impl ArchipelagoData {
    pub(crate) fn add_blue_orb(&mut self) {
        self.blue_orbs = (self.blue_orbs + 1).min(14);
    }

    pub(crate) fn add_purple_orb(&mut self) {
        self.purple_orbs = (self.purple_orbs + 1).min(10);
        if let Some(mappings) = MAPPING.read().unwrap().as_ref() {
            if !mappings.devil_trigger_mode {
                self.dt_unlocked = true;
            }
        }
    }

    pub(crate) fn add_dt(&mut self) {
        if let Some(mappings) = MAPPING.read().unwrap().as_ref() {
            if mappings.devil_trigger_mode {
                self.dt_unlocked = true;
            }
            if !mappings.purple_orb_mode {
                self.purple_orbs = (self.purple_orbs + 3).min(10);
            }
        }

    }

    pub(crate) fn add_gun_level(&mut self, gun_index: usize) {
        self.gun_levels[gun_index] = (self.gun_levels[gun_index] + 1).min(2);
    }

    pub(crate) fn add_style_level(&mut self, style: Style) {
        /*
        0 = not unlocked
        1 = level 1
        2 = level 2
        3 = level 3

        In terms of style levelling, the game considers 0 to be level 1
         */
        self.style_levels[style.index()] = (self.style_levels[style.index()] + 1).min(3);
    }

    pub(crate) fn get_style_unlocked(&self) -> [bool; 4] {
        let mut style_table = [false, false, false, false];
        for i in 0..4 {
            style_table[i] = self.style_levels[i] > 0;
        }
        style_table
    }

    // Sword, Gun, Trick, Royal for style_levels
    // Trick, Sword, Gun, Royal - Default order
    fn get_style_level_array(&self) -> [u32; 6] {
        let mut style_levels = [0, 0, 0, 0, 0, 0];
        style_levels[0] = (self.style_levels[1] - 1).max(0) as u32;
        style_levels[1] = (self.style_levels[2] - 1).max(0) as u32;
        style_levels[2] = (self.style_levels[0] - 1).max(0) as u32;
        style_levels[3] = (self.style_levels[3] - 1).max(0) as u32;

        style_levels
    }

    pub(crate) fn add_stinger_level(&mut self) {
        self.stinger_level = (self.stinger_level + 1).min(2);
    }

    pub(crate) fn add_jet_stream_level(&mut self) {
        self.jet_stream_level = (self.jet_stream_level + 1).min(2);
    }
    pub(crate) fn add_reverb_level(&mut self) {
        self.reverb_level = (self.reverb_level + 1).min(2);
    }
}

#[repr(C)]
pub struct SessionData {
    pub(crate) mission: u32,
    pub(crate) other_mission: u32, // Don't know what this does, copies from mission after a second
    pub(crate) room: i32,          // Should be right?
    pub(crate) difficulty: u32,
    pub(crate) hoh: bool,
    pub _unknown2: u8,
    tutorial: bool,
    gold_orb_mode: bool,
    pub(crate) char: u8,
    pub(crate) _unknown3: [u8; 7],
    bloody_palace: bool,
    _unknown4: [u8; 15],
    red_orbs: u32,
    pub(crate) items: [u8; 20],
    unknown5: [u8; 2],
    unlocks: [bool; 14],
    unknown6: [u8; 48],
    pub(crate) weapons: [u8; 8],
    unknown7: [u8; 20],
    pub(crate) ranged_weapon_levels: [u32; 5],
    pub(crate) unknown8: [u8; 20],
    pub melee_index: u32,
    pub gun_index: u32,
    costume: u8,
    pub unlocked_dt: bool,
    pub unknown9: [u8; 2],
    pub max_hp: f32,
    pub max_magic: f32,
    pub style: u32,
    style_levels: [u32; 6],
    style_xp: [f32; 6],
    pub(crate) expertise: [u32; 8],
}

/// Error type for session access
#[derive(Debug)]
pub enum SessionError {
    NotUsable, // If a save slot has not been loaded for whatever reason
}

static SESSION_PTR: LazyLock<usize> = LazyLock::new(|| *DMC3_ADDRESS + GAME_SESSION_DATA);

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

/// Get current difficulty
pub fn get_difficulty() -> Difficulty {
    Difficulty::from_repr(
        with_session_read(|s| if s.hoh { return 5 } else { s.difficulty }).unwrap() as usize,
    )
    .unwrap()
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
        let char_data_ptr: usize = read_data_from_address(*DMC3_ADDRESS + ACTIVE_CHAR_DATA);
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
        let char_data_ptr: usize = read_data_from_address(*DMC3_ADDRESS + ACTIVE_CHAR_DATA);
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
        match ARCHIPELAGO_DATA.read() {
            Ok(data) => {
                s.max_hp = f32::min(BASE_HP + (data.blue_orbs as f32 * ONE_ORB), MAX_HP);
                log::debug!("New HP is: {}", s.max_hp);
                if data.dt_unlocked {
                    s.max_magic = f32::min(data.purple_orbs as f32 * ONE_ORB, MAX_MAGIC);
                    log::debug!("New Magic is: {}", s.max_magic);
                }
            }
            Err(err) => {
                log::error!("Failed to read data from ARCHIPELAGO_DATA: {}", err);
            }
        }
    })
    .unwrap();
}

pub(crate) fn kill_dante() {
    let char_data_ptr: usize = read_data_from_address(*DMC3_ADDRESS + ACTIVE_CHAR_DATA);
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

pub(crate) fn set_gun_levels() {
    log::debug!("Setting gun levels");
    with_session(|s| match ARCHIPELAGO_DATA.read() {
        Ok(data) => {
            for i in 0..s.ranged_weapon_levels.len() {
                s.ranged_weapon_levels[i] = data.gun_levels[i];
            }
        }
        Err(err) => {
            log::error!("Failed to read data from ARCHIPELAGO_DATA: {}", err);
        }
    })
    .expect("Unable to edit session data");
    let gun_upgrade_offset = 0x3FEC;
    let char_data_ptr: usize = read_data_from_address(*DMC3_ADDRESS + ACTIVE_CHAR_DATA);
    if char_data_ptr != 0 {
        unsafe {
            let mut gun_levels =
                read_unaligned((char_data_ptr + gun_upgrade_offset) as *mut [u32; 10]);
            match ARCHIPELAGO_DATA.read() {
                Ok(data) => {
                    for i in 0..(*GUN_NAMES).len() {
                        gun_levels[get_weapon_id(&*GUN_NAMES[i]) as usize] += data.gun_levels[i];
                    }
                }
                Err(err) => {
                    log::error!("Failed to read data from ARCHIPELAGO_DATA: {}", err);
                }
            }
            write_unaligned(
                (char_data_ptr + gun_upgrade_offset) as *mut [u32; 10],
                gun_levels,
            )
        }
    }
}

pub(crate) fn set_style_levels() {
    with_session(|s| match ARCHIPELAGO_DATA.read() {
        Ok(data) => {
            s.style_levels = data.get_style_level_array();
        }
        Err(err) => {
            log::error!("Failed to read data from ARCHIPELAGO_DATA: {}", err);
        }
    })
    .unwrap();
}

pub(crate) fn apply_style_levels(style: Style) {
    //set_style_levels();
    let char_data_ptr: usize = read_data_from_address(*DMC3_ADDRESS + ACTIVE_CHAR_DATA);
    if char_data_ptr != 0 {
        unsafe {
            const LEVEL_1_XP: f32 = 30000f32; // XP To get to LV2
            const LEVEL_2_XP: f32 = 99999f32; // LV2 -> LV3
            const PTR_3: usize = 0xCF2548; //0xC90E28 + 0x18;
            let equipped_style = read_data_from_address::<u32>(char_data_ptr + 0x6338) as usize;
            if style.get_internal_order() == equipped_style {
                let level = read_data_from_address::<u32>(char_data_ptr + 0x6358);
                let ptr = read_data_from_address::<usize>(*DMC3_ADDRESS + PTR_3);
                if ptr != 0 {
                    match level {
                        0 => {
                            ORIGINAL_GIVE_STYLE_XP.get().unwrap()(ptr, LEVEL_1_XP);
                        }
                        1 => {
                            ORIGINAL_GIVE_STYLE_XP.get().unwrap()(ptr, LEVEL_2_XP);
                        }
                        2 => {
                            log::debug!("Style {} is max level", style);
                        }
                        _ => {
                            log::error!("Unknown level: {}", level);
                        }
                    }
                } else {
                    log::error!("ptr 3 was 0: {:#X}", *DMC3_ADDRESS + PTR_3)
                }
            }
        }
    }
}
