use crate::constants::{
    get_items_by_category, get_weapon_id, Difficulty, ItemCategory, BASE_HP, GUN_NAMES, ITEM_MAP, ITEM_OFFSET_MAP,
    MAX_HP, MAX_MAGIC, MELEE_NAMES, ONE_ORB,
};
use crate::hook::ORIGINAL_GIVE_STYLE_XP;
use crate::mapping::MAPPING;
use crate::utilities;
use crate::utilities::{get_inv_address, read_data_from_address, DMC3_ADDRESS};
use randomizer_utilities::replace_single_byte;
use std::collections::HashSet;
use std::ptr::{read_unaligned, write_unaligned};
use std::sync::{LazyLock, RwLock};

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
    // Beast uppercut -> Rising dragon
    pub(crate) beowulf_level: u8,
    pub(crate) items: HashSet<String>,
    pub(crate) skills: HashSet<usize>,
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
    pub fn add_item(&mut self, item: String) {
        self.items.insert(item);
    }

    pub fn add_skill(&mut self, skill_id: usize) {
        self.skills.insert(skill_id);
    }

    pub(crate) fn add_blue_orb(&mut self) {
        self.blue_orbs = (self.blue_orbs + 1).min(14);
    }

    pub(crate) fn add_purple_orb(&mut self) {
        self.purple_orbs = (self.purple_orbs + 1).min(10);
        if let Some(mappings) = MAPPING.read().unwrap().as_ref()
            && !mappings.devil_trigger_mode
        {
            self.dt_unlocked = true;
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
        for style_idx in 0..4 {
            style_table[style_idx] = self.style_levels[style_idx] > 0;
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

    pub(crate) fn add_beowulf_level(&mut self) {
        self.beowulf_level = (self.beowulf_level + 1).min(2);
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
    pub(crate) red_orbs: u32,
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
        with_session_read(|s| if s.hoh { 5 } else { s.difficulty }).unwrap() as usize,
    )
    .unwrap()
}

const CHARACTER_DATA: usize = 0xC90E30;

pub(crate) fn give_magic(magic_val: f32, arch_data: &ArchipelagoData) {
    let base = *DMC3_ADDRESS;
    if arch_data.dt_unlocked {
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
            if let Some(char_data_ptr) = utilities::get_active_char_address() {
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
}

pub(crate) fn give_hp(life_value: f32) {
    let base = *DMC3_ADDRESS;
    unsafe {
        write_unaligned(
            (base + CHARACTER_DATA + 0x16C + 0x64) as *mut f32,
            read_unaligned((base + CHARACTER_DATA + 0x16C + 0x64) as *mut f32) + life_value,
        ); // Life
        write_unaligned(
            (base + CHARACTER_DATA + 0x16C + 0x68) as *mut f32,
            read_unaligned((base + CHARACTER_DATA + 0x16C + 0x68) as *mut f32) + life_value,
        ); // Max life
        if let Some(char_data_ptr) = utilities::get_active_char_address() {
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
    if let Some(inv_address) = get_inv_address() {
        unsafe {
            replace_single_byte(
                inv_address + *ITEM_OFFSET_MAP.get(item_name).unwrap() as usize,
                has_item as u8,
            )
        };
        if set_flag {
            set_loc_chk_flg(item_name, has_item);
        }
    }
}

const LOCATION_FLAGS: usize = 0xc90e28;
pub fn set_loc_chk_flg(item_name: &str, set_flag: bool) {
    let ptr: usize = read_data_from_address(*DMC3_ADDRESS + LOCATION_FLAGS);
    let item_id: i32 = *ITEM_MAP.get_by_left(item_name).unwrap() as i32;
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
    let item_id: i32 = *ITEM_MAP.get_by_left(item_name).unwrap() as i32;
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
                } else {
                    s.max_magic = 0.0
                }
                log::debug!("New Magic is: {}", s.max_magic);
            }
            Err(err) => {
                log::error!("Failed to read data from ARCHIPELAGO_DATA: {}", err);
            }
        }
    })
    .unwrap();
}

pub(crate) fn hurt_dante() {
    let damage_fraction: f32 = match get_difficulty() {
        Difficulty::Easy => 1.0 / 4.0,
        Difficulty::Normal => 1.0 / 3.0,
        Difficulty::Hard => 1.0 / 2.0,
        Difficulty::VeryHard => 2.0 / 3.0,
        Difficulty::DanteMustDie => 5.0 / 6.0,
        // Insta kill
        Difficulty::HeavenOrHell => 1.0,
    };
    if let Some(char_data_ptr) = utilities::get_active_char_address() {
        let hp_addr = char_data_ptr + 0x411C;
        unsafe {
            let max_hp = read_unaligned((char_data_ptr + 0x40EC) as *mut f32);
            write_unaligned(
                hp_addr as *mut f32,
                f32::max(
                    read_unaligned(hp_addr as *const f32) - (max_hp * damage_fraction),
                    0.0,
                ),
            );
        }
    }
}

pub(crate) fn kill_dante() {
    if let Some(char_data_ptr) = utilities::get_active_char_address() {
        unsafe {
            write_unaligned((char_data_ptr + 0x411C) as *mut f32, 0.0);
        }
    }
}

pub fn set_session_weapons() {
    if let Ok(data) = ARCHIPELAGO_DATA.read() {
        with_session(|s| {
            for weapon in get_items_by_category(ItemCategory::Weapon) {
                if data.items.contains(&weapon.to_string()) {
                    let weapon_id = get_weapon_id(weapon);
                    if MELEE_NAMES.contains(&weapon)
                        && s.weapons[0] != weapon_id
                        && s.weapons[1] == 0xFF
                    {
                        log::debug!("Inserting {} into second melee slot", weapon);
                        s.weapons[1] = weapon_id;
                    }

                    if GUN_NAMES.contains(&weapon)
                        && s.weapons[2] != weapon_id
                        && s.weapons[3] == 0xFF
                    {
                        log::debug!("Inserting {} into second gun slot", weapon);
                        s.weapons[3] = weapon_id;
                    }
                }
            }
        })
        .unwrap();
    }
}
//const WEAPON_SLOT: usize = 0x045FF2D8;
pub(crate) fn set_weapons_in_inv() {
    let mut flag;
    if let Ok(data) = ARCHIPELAGO_DATA.read() {
        for weapon in get_items_by_category(ItemCategory::Weapon) {
            if data.items.contains(&weapon.to_string()) {
                flag = true;
                log::debug!("Adding weapon/style to inventory {}", weapon);
            } else {
                flag = false;
            }
            set_item(weapon, flag, true);
        }
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
    const GUN_UPGRADE_OFFSET: usize = 0x3FEC;
    if let Some(char_data_ptr) = utilities::get_active_char_address() {
        unsafe {
            let mut gun_levels =
                read_unaligned((char_data_ptr + GUN_UPGRADE_OFFSET) as *mut [u32; 10]);
            match ARCHIPELAGO_DATA.read() {
                Ok(data) => {
                    for i in 0..(*GUN_NAMES).len() {
                        gun_levels[get_weapon_id(GUN_NAMES[i]) as usize] += data.gun_levels[i];
                    }
                }
                Err(err) => {
                    log::error!("Failed to read data from ARCHIPELAGO_DATA: {}", err);
                }
            }
            write_unaligned(
                (char_data_ptr + GUN_UPGRADE_OFFSET) as *mut [u32; 10],
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
    if let Some(char_data_ptr) = utilities::get_active_char_address() {
        unsafe {
            const LEVEL_1_XP: f32 = 30000f32; // XP To get to LV2
            const LEVEL_2_XP: f32 = 99999f32; // LV2 -> LV3
            let equipped_style = read_data_from_address::<u32>(char_data_ptr + 0x6338) as usize;
            if style.get_internal_order() == equipped_style {
                let level = read_data_from_address::<u32>(char_data_ptr + 0x6358);
                if let Some(char_data_ptr) = utilities::get_active_char_address() {
                    match level {
                        0 => {
                            ORIGINAL_GIVE_STYLE_XP.get().unwrap()(char_data_ptr, LEVEL_1_XP);
                        }
                        1 => {
                            ORIGINAL_GIVE_STYLE_XP.get().unwrap()(char_data_ptr, LEVEL_2_XP);
                        }
                        2 => {
                            log::debug!("Style {} is max level", style);
                        }
                        _ => {
                            log::error!("Unknown level: {}", level);
                        }
                    }
                }
            }
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct TotalRankings {
    pub easy_ranking: [u8; 20],
    pub normal_ranking: [u8; 20],
    pub hard_ranking: [u8; 20],
    pub very_hard_ranking: [u8; 20],
    pub dmd_ranking: [u8; 20],
    pub hoh_ranking: [u8; 20],
}

static RANKING_PTR: LazyLock<usize> = LazyLock::new(|| *DMC3_ADDRESS + 0xC8F8E5);

pub fn with_rankings_read<F, R>(f: F) -> Result<R, SessionError>
where
    F: FnOnce(&TotalRankings) -> R,
{
    let addr = *RANKING_PTR;
    unsafe {
        let s = &*(addr as *const TotalRankings);
        Ok(f(s))
    }
}
