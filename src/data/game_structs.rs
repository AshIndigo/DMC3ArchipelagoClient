use crate::constants::{Character, Difficulty};
use crate::hooks::hook::SCENE;
use crate::utilities::DMC3_ADDRESS;
use randomizer_utilities::read_data_from_address;
use std::sync::atomic::Ordering;

/// Error type for accessing data
#[derive(Debug)]
pub enum GameDataError {
    NotUsable, // If the requested data is unavailable
}

pub trait GameData: Sized {
    fn ptr() -> usize;
    fn is_valid() -> bool {
        read_data_from_address::<usize>(Self::ptr()) != 0
    }

    fn with_mut<F, R>(f: F) -> Result<R, GameDataError>
    where
        F: FnOnce(&mut Self) -> R,
    {
        unsafe {
            if !Self::is_valid() {
                return Err(GameDataError::NotUsable);
            }

            let ptr = Self::ptr() as *mut Self;
            Ok(f(&mut *ptr))
        }
    }

    fn with_read<F, R>(f: F) -> Result<R, GameDataError>
    where
        F: FnOnce(&Self) -> R,
    {
        unsafe {
            if !Self::is_valid() {
                return Err(GameDataError::NotUsable);
            }

            let ptr = Self::ptr() as *const Self;
            Ok(f(&*ptr))
        }
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
    pub(crate) character: Character,
    pub(crate) _unknown3: [u8; 7],
    bloody_palace: bool,
    _unknown4: [u8; 15],
    pub(crate) red_orbs: i32,
    pub(crate) items: [u8; 20],
    unknown5: [u8; 2],
    pub(crate) weapon_style_unlocks: [bool; 14],
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
    pub(crate) style_levels: [u32; 6],
    style_xp: [f32; 6],
    pub(crate) expertise: [u32; 8],
}

impl GameData for SessionData {
    fn ptr() -> usize {
        const GAME_SESSION_DATA: usize = 0xC8F250;
        *DMC3_ADDRESS + GAME_SESSION_DATA
    }
}

#[repr(C)]
pub struct MissionData {
    unknown0: [u8; 56],
    pub(crate) red_orbs: i32,
    pub(crate) items: [u8; 62],
    pub(crate) bought_items: [u8; 8],
    unknown1: [u8; 38],
    frame_count: u32,
    damage_taken: u32,
    orbs_collected: u32,
    items_used: u32,
    kill_count: u32,
    unknown2: [u8; 4],
}

impl GameData for MissionData {
    fn ptr() -> usize {
        const MISSION_CHARACTER_DATA: usize = 0xC90E30;
        *DMC3_ADDRESS + MISSION_CHARACTER_DATA
    }

    fn with_mut<F, R>(f: F) -> Result<R, GameDataError>
    where
        F: FnOnce(&mut Self) -> R,
    {
        unsafe {
            if !Self::is_valid() {
                return Err(GameDataError::NotUsable);
            }

            let ptr = *(Self::ptr() as *mut *mut Self);
            let s = &mut *ptr;

            Ok(f(s))
        }
    }

    fn with_read<F, R>(f: F) -> Result<R, GameDataError>
    where
        F: FnOnce(&Self) -> R,
    {
        unsafe {
            if !Self::is_valid() {
                return Err(GameDataError::NotUsable);
            }

            let ptr = *(Self::ptr() as *const *const Self);
            let s = &*ptr;

            Ok(f(s))
        }
    }
}

#[repr(C)]
pub struct CharacterData {
    // I could base this off of Crimson's, but most of the data in this struct is not needed for my purposes
    _unknown1: [u8; 16056],
    pub(crate) magic: f32,
    pub(crate) max_magic: f32,
    _unknown2: [u8; 300],
    pub(crate) expertise: [u32; 8],
    _unknown3: [u8; 32],
    queued_expertise: [u32; 8],
    _unknown4: [u8; 160],
    pub(crate) max_hp: f32, // 0x40EC
    _unknown5: [u8; 44],
    pub(crate) hp: f32,
    _unknown6: [u8; 8728],
    pub(crate) style: u32,
    _unknown7: [u8; 28],
    pub(crate) style_level: u32,
    _unknown8: [u8; 8],
    pub(crate) exp_points: f32,
    _unknown9: [u8; 372],
    pub(crate) weapon_levels: [u32; 5],
}

impl GameData for CharacterData {
    fn ptr() -> usize {
        const ACTIVE_CHAR: usize = 0xCF2548;
        *DMC3_ADDRESS + ACTIVE_CHAR
    }

    fn with_mut<F, R>(f: F) -> Result<R, GameDataError>
    where
        F: FnOnce(&mut Self) -> R,
    {
        unsafe {
            if !Self::is_valid() {
                return Err(GameDataError::NotUsable);
            }

            let ptr = *(Self::ptr() as *mut *mut Self);
            let s = &mut *ptr;

            Ok(f(s))
        }
    }

    fn with_read<F, R>(f: F) -> Result<R, GameDataError>
    where
        F: FnOnce(&Self) -> R,
    {
        unsafe {
            if !Self::is_valid() {
                return Err(GameDataError::NotUsable);
            }

            let ptr = *(Self::ptr() as *const *const Self);
            let s = &*ptr;

            Ok(f(s))
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

impl TotalRankings {
    pub fn get_ranking_for_difficulty(&self, difficulty: Difficulty) -> [u8; 20] {
        match difficulty {
            Difficulty::Easy => self.easy_ranking,
            Difficulty::Normal => self.normal_ranking,
            Difficulty::Hard => self.hard_ranking,
            Difficulty::VeryHard => self.very_hard_ranking,
            Difficulty::DanteMustDie => self.dmd_ranking,
            Difficulty::HeavenOrHell => self.hoh_ranking,
        }
    }
}

impl GameData for TotalRankings {
    fn ptr() -> usize {
        const TOTAL_RANKINGS: usize = 0xC8F8E5;
        *DMC3_ADDRESS + TOTAL_RANKINGS
    }

    fn is_valid() -> bool {
        true
    }
}

#[repr(C)]
pub struct QueuedMissionActorData {
    weapons: [u8; 5],
    _unknown0: [u8; 20],
    pub weapon_levels: [u32; 5],
    _unknown1: [u32; 7],
    _padding: [u8; 3],
    hp: f32,
    magic: f32,
    style: u32,
    style_level: [u32; 6],
    style_exp: [f32; 6],
    expertise: [u32; 8],
}

impl GameData for QueuedMissionActorData {
    fn ptr() -> usize {
        read_data_from_address::<usize>(MissionData::ptr()) + 0xC0
    }

    fn is_valid() -> bool {
        MissionData::is_valid()
    }
}

#[repr(C)]
pub struct ActiveMissionActorData {
    pub(crate) equipped_weapons: [u8; 5],
    unknown0: [u8; 3],
    pub weapon_levels: [u32; 5],
    melee_index: u32,
    ranged_index: u32,
    active_weapon: u8,
    unknown1: [u8; 3],
    active_model: u32, // Some internal thing, doesnt matter
    devil_trigger: bool,
    unknown2: [u8; 3],
    mode: i32,
    costume: u8,
    unknown3: [u8; 3],
    style: u32,
    style_level: u32,
    pub(crate) expertise: [u32; 8],
    exp_points: f32,
    pub(crate) hp: f32,
    pub(crate) max_hp: f32,
    pub(crate) magic: f32,
    pub(crate) max_magic: f32,
}

impl GameData for ActiveMissionActorData {
    fn ptr() -> usize {
        read_data_from_address::<usize>(MissionData::ptr()) + 0x16C
    }

    fn is_valid() -> bool {
        MissionData::is_valid()
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct EventData {
    unknown: [u8; 24],
    room: u32,
    position: u32,
    pub(crate) event: Event,
    subevent: u32,
    screen: u32,
    next_screen: u32,
}
#[allow(dead_code)]
#[repr(u32)]
#[derive(Debug, strum_macros::Display, strum_macros::FromRepr, PartialEq)]
pub enum Event {
    Init,
    Main,
    Teleport,
    Pause,
    Status,
    Options,
    Death,
    Item,
    Message,
    Customize,
    Save,
    Delete,
    End,
}

impl GameData for EventData {
    fn ptr() -> usize {
        *DMC3_ADDRESS + 0xC90E10
    }

    fn is_valid() -> bool {
        unsafe {
            if SCENE.load(Ordering::SeqCst) != 5 {
                return false;
            }
            let ptr = *(Self::ptr() as *const *const *const u8);
            if ptr.is_null() || (*ptr.add(8)).is_null() {
                return false;
            }
        }
        true
    }

    fn with_mut<F, R>(f: F) -> Result<R, GameDataError>
    where
        F: FnOnce(&mut Self) -> R,
    {
        unsafe {
            if !Self::is_valid() {
                return Err(GameDataError::NotUsable);
            }

            let ptr = *(Self::ptr() as *const *const *const u8);

            let event_data = &mut *((*ptr.add(8)) as *mut EventData);

            Ok(f(event_data))
        }
    }

    fn with_read<F, R>(f: F) -> Result<R, GameDataError>
    where
        F: FnOnce(&Self) -> R,
    {
        unsafe {
            if !Self::is_valid() {
                return Err(GameDataError::NotUsable);
            }

            let ptr = *(Self::ptr() as *const *const *const u8);
            let event_data = &*((*ptr.add(8)) as *const EventData);

            Ok(f(event_data))
        }
    }
}
