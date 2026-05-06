use crate::constants::{
    BASE_HP, Difficulty, GUN_NAMES, ITEM_MAP, ItemCategory, MAX_HP, MAX_MAGIC, MELEE_NAMES,
    ONE_ORB, Style, get_items_by_category, get_unlocked_weapon_id, get_weapon_id,
};
use crate::data::game_structs::{
    ActiveMissionActorData, CharacterData, GameData, MissionData, SessionData,
};
use crate::hooks::hook::ORIGINAL_GIVE_STYLE_XP;
use crate::mapping::MAPPING;
use crate::utilities::{DMC3_ADDRESS, read_data_from_address};
use archipelago_rs::Item;
use randomizer_utilities::replace_single_byte;
use std::collections::HashSet;
use std::sync::{LazyLock, RwLock};

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

    pub(crate) fn reset_gun_levels(&mut self) {
        self.gun_levels = [0; 5];
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

    pub(crate) fn reset_style_levels(&mut self) {
        self.style_levels = [0; 4];
    }

    pub(crate) fn get_style_unlocked(&self) -> [bool; 4] {
        let mut style_table = [false; 4];
        for (out, level) in style_table.iter_mut().zip(self.style_levels.iter()) {
            *out = *level > 0;
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

/// Get current mission
pub fn get_mission() -> u32 {
    SessionData::with_read(|s| s.mission).unwrap_or_else(|_| {
        log::debug!("Attempting to get mission before session data is ready");
        0
    })
}

/// Get current room
pub fn get_room() -> i32 {
    SessionData::with_read(|s| s.room).unwrap()
}

/// Get current difficulty
pub fn get_difficulty() -> Difficulty {
    Difficulty::from_repr(
        SessionData::with_read(|s| if s.hoh { 5 } else { s.difficulty }).unwrap() as usize,
    )
    .unwrap()
}

pub(crate) fn give_magic(magic_val: f32, data: &ArchipelagoData) {
    log::debug!("Supplying added Magic");
    let _ = SessionData::with_mut(|s| {
        if data.dt_unlocked {
            s.max_magic = f32::min(data.purple_orbs as f32 * ONE_ORB, MAX_MAGIC);
        } else {
            s.max_magic = 0.0
        }
    });
    let _ = ActiveMissionActorData::with_mut(|d| {
        d.magic += magic_val;
        if data.dt_unlocked {
            d.max_magic = f32::min(data.purple_orbs as f32 * ONE_ORB, MAX_MAGIC);
        } else {
            d.max_magic = 0.0
        }
    });
    let _ = CharacterData::with_mut(|d| {
        d.magic += magic_val;
        if data.dt_unlocked {
            d.max_magic = f32::min(data.purple_orbs as f32 * ONE_ORB, MAX_MAGIC);
        } else {
            d.max_magic = 0.0
        }
    });
}

pub(crate) fn give_hp(life_value: f32, data: &ArchipelagoData) {
    log::debug!("Supplying added HP");
    let _ = ActiveMissionActorData::with_mut(|d| {
        d.hp += life_value;
        d.max_hp = f32::min(BASE_HP + (data.blue_orbs as f32 * ONE_ORB), MAX_HP);
    });
    let _ = CharacterData::with_mut(|f| {
        f.hp += life_value;
        f.max_hp = f32::min(BASE_HP + (data.blue_orbs as f32 * ONE_ORB), MAX_HP);
    });
}

/// Use for weapons/key items
pub(crate) fn set_item(item_name: &str, has_item: bool, set_flag: bool) {
    log::debug!("Setting item {} to {}", item_name, has_item);
    let _ = MissionData::with_mut(|m| {
        m.items[*ITEM_MAP.get_by_left(item_name).unwrap() as usize] = has_item as u8;
        if set_flag {
            set_loc_chk_flg(item_name, has_item);
        }
    });
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
    SessionData::with_mut(|s| {
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
    let _ = CharacterData::with_mut(|c| {
        c.hp = f32::max(c.hp - (c.max_hp * damage_fraction), 0.0);
    });
}

pub(crate) fn kill_dante() {
    let _ = CharacterData::with_mut(|c| {
        c.hp = 0.0;
    });
}

pub fn set_session_weapons() {
    if let Ok(data) = ARCHIPELAGO_DATA.read() {
        SessionData::with_mut(|s| {
            for weapon in get_items_by_category(ItemCategory::Weapon) {
                let weapon_id = get_weapon_id(weapon);
                if data.items.contains(weapon) {
                    if MELEE_NAMES.contains(&weapon) {
                        // First slot
                        if s.weapons[0] == 0xFF {
                            log::debug!("Inserting {} into first melee slot", weapon);
                            s.weapons[0] = weapon_id;
                        }
                        // Second slot
                        if s.weapons[0] != weapon_id && s.weapons[1] == 0xFF {
                            log::debug!("Inserting {} into second melee slot", weapon);
                            s.weapons[1] = weapon_id;
                        }
                    }
                    if GUN_NAMES.contains(&weapon) {
                        // First slot
                        if s.weapons[2] == 0xFF {
                            log::debug!("Inserting {} into first gun slot", weapon);
                            s.weapons[2] = weapon_id;
                        }
                        // Second slot
                        if s.weapons[2] != weapon_id && s.weapons[3] == 0xFF {
                            log::debug!("Inserting {} into second gun slot", weapon);
                            s.weapons[3] = weapon_id;
                        }
                    }
                }
                s.weapon_style_unlocks[get_unlocked_weapon_id(weapon) as usize] =
                    data.items.contains(weapon);
            }
        })
        .unwrap();
    }
}

pub(crate) fn set_weapons_in_inv(data: &ArchipelagoData) {
    for weapon in get_items_by_category(ItemCategory::Weapon) {
        set_item(weapon, data.items.contains(weapon), true);
    }
}

pub(crate) fn set_gun_levels(data: &ArchipelagoData) {
    log::debug!("Setting gun levels");
    SessionData::with_mut(|s| {
        for i in 0..s.ranged_weapon_levels.len() {
            s.ranged_weapon_levels[i] = data.gun_levels[i];
        }
    })
    .expect("Unable to edit session data");
    // TODO Sort out in game gun level changes
    // let _ = CharacterData::with_mut(|c| {
    //     c.weapon_levels = data.gun_levels;
    //     // TODO for whatever reason this field only keeps track of equipped weapons
    // });

    // let _ = MissionData::with_mut(|c| {
    //     log::debug!("MD: {:#X}", read_data_from_address::<usize>(MissionData::ptr()));
    //     log::debug!("Gun?: {:?}", c.gun_stuff);
    //     //c.levels = data.gun_levels.map(|f| f as u8);
    // });
    //
    // let _ = ActiveMissionActorData::with_mut(|a| {
    //     log::debug!("Maybe levels: {:?}", a.equipped_weapons)
    // });
}

pub(crate) fn set_style_levels() {
    SessionData::with_mut(|s| match ARCHIPELAGO_DATA.read() {
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
    unsafe {
        // Note these are based on default values. If I randomized them I will to update this section
        const LEVEL_1_XP: f32 = 30000f32; // XP To get to LV2
        const LEVEL_2_XP: f32 = 99999f32; // LV2 -> LV3
        let equipped_style = CharacterData::with_read(|c| c.style).unwrap_or_else(|_| {
            log::error!("Unable to get current style");
            0
        });
        if style.get_internal_order_index() == equipped_style as usize {
            let level = CharacterData::with_read(|c| c.style_level).unwrap_or_else(|_| {
                log::error!("Unable to get current style level");
                0
            });
            match level {
                0 => {
                    ORIGINAL_GIVE_STYLE_XP.get().unwrap()(CharacterData::ptr(), LEVEL_1_XP);
                }
                1 => {
                    ORIGINAL_GIVE_STYLE_XP.get().unwrap()(CharacterData::ptr(), LEVEL_2_XP);
                }
                2 => {
                    log::debug!("Style {style} is max level");
                }
                _ => {
                    log::error!("Unknown {style} level: {level}");
                }
            }
        }
    }
}

pub(crate) fn add_consumable(item: Item) {
    log::debug!("Adding Consumable item {}", item);
    // Add to mission inv
    let _ = MissionData::with_mut(|m| {
        m.items[item.id() as usize] += 1;
    });
    SessionData::with_mut(|session| {
        session.items[item.id() as usize] += 1;
    })
    .unwrap();
}

pub(crate) fn give_red_orbs(orbs: i32) {
    log::debug!("Giving {} orbs", orbs);
    if SessionData::with_mut(|session| session.red_orbs += orbs).is_err() {
        log::warn!("Failed to give red orbs for session data");
    };
    if MissionData::with_mut(|m| m.red_orbs += orbs).is_err() {};
}
