use randomizer_utilities::cache::DATA_PACKAGE;
use crate::skill_manager::ID_SKILL_MAP;
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::sync::LazyLock;
use randomizer_utilities::mapping_utilities::GameConfig;

pub type BasicNothingFunc = unsafe extern "system" fn();

pub(crate) const DUMMY_ID: LazyLock<u32> = LazyLock::new(|| *ITEM_ID_MAP.get("Dummy").unwrap());

pub(crate) const REMOTE_ID: LazyLock<u32> = LazyLock::new(|| *ITEM_ID_MAP.get("Remote").unwrap());

pub struct DMC3Config;
pub const GAME_NAME: &str = "Devil May Cry 3";

impl GameConfig for DMC3Config {
    const REMOTE_ID: u32 = 0x26;
    const GAME_NAME: &'static str = GAME_NAME;
}


// DMC3 Offsets+Functions - Offsets are from 2022 DDMK's version

pub const ITEM_MODE_TABLE: usize = 0x1B4534; // This is actually a constant, we like this one

pub const ONE_ORB: f32 = 1000.0; // One Blue/Purple orb is worth 1000 "points"
pub const BASE_HP: f32 = 6.0 * ONE_ORB;
pub const MAX_HP: f32 = 20000.0;
pub const MAX_MAGIC: f32 = 10000.0;
pub struct Item {
    pub id: u32,
    pub name: &'static str,
    pub offset: Option<u8>, // Inventory offset
    pub category: ItemCategory,
    pub mission: Option<u32>, // Mission the key item is used in, typically the same that it is acquired in
    pub _max_amount: Option<i32>, // Max amount of a consumable
    pub _value: Option<i32>,  // Value of an orb, used only for red orbs
}

#[derive(PartialEq)]
pub(crate) enum ItemCategory {
    Key,
    Consumable,
    Weapon,
    RedOrb, // Red orbs are special...
    Misc,
}

pub(crate) const ALL_ITEMS: [Item; 58] = [
    Item {
        id: 0x00,
        name: "Red Orb - 1",
        offset: Some(0x38),
        category: ItemCategory::RedOrb,
        mission: None,
        _max_amount: Some(999999), // Is what fits on screen, could theoretically go up to MAX_INT
        _value: Some(1),
    },
    Item {
        id: 0x01,
        name: "Red Orb - 5",
        offset: Some(0x38),
        category: ItemCategory::RedOrb,
        mission: None,
        _max_amount: Some(999999),
        _value: Some(5),
    },
    Item {
        id: 0x02,
        name: "Red Orb - 20",
        offset: Some(0x38),
        category: ItemCategory::RedOrb,
        mission: None,
        _max_amount: Some(999999),
        _value: Some(1),
    },
    Item {
        id: 0x03,
        name: "Red Orb - 100",
        offset: Some(0x38),
        category: ItemCategory::RedOrb,
        mission: None,
        _max_amount: Some(999999),
        _value: Some(100),
    },
    Item {
        id: 0x04,
        name: "Red Orb - 1000",
        offset: Some(0x38),
        category: ItemCategory::RedOrb,
        mission: None,
        _max_amount: Some(999999),
        _value: Some(1000),
    },
    Item {
        id: 0x05,
        name: "Gold Orb",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: Some(3),
        _value: None,
    },
    Item {
        id: 0x06,
        name: "Yellow Orb",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: Some(99),
        _value: None,
    },
    Item {
        id: 0x07,
        name: "Blue Orb",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x08,
        name: "Purple Orb",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x09,
        name: "Blue Orb Fragment",
        offset: Some(0x45),
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: Some(4),
        _value: None,
    },
    Item {
        id: 0x0A,
        name: "Green Orb - Small",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x0B,
        name: "Green Orb - Medium",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x0C,
        name: "Green Orb - Large",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x0D,
        name: "Unknown D",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x0E,
        name: "Unknown E",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x0F,
        name: "Unknown F",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x10,
        name: "Vital Star L",
        offset: Some(0x4C),
        category: ItemCategory::Consumable,
        mission: None,
        _max_amount: Some(30),
        _value: None,
    },
    Item {
        id: 0x11,
        name: "Vital Star S",
        offset: Some(0x4D),
        category: ItemCategory::Consumable,
        mission: None,
        _max_amount: Some(30),
        _value: None,
    },
    Item {
        id: 0x12,
        name: "Devil Star",
        offset: Some(0x4E),
        category: ItemCategory::Consumable,
        mission: None,
        _max_amount: Some(10),
        _value: None,
    },
    Item {
        id: 0x13,
        name: "Holy Water",
        offset: Some(0x4F),
        category: ItemCategory::Consumable,
        mission: None,
        _max_amount: Some(30),
        _value: None,
    },
    Item {
        id: 0x14,
        name: "Scent of Fear", // Scent of Fear test item, old dummy
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x15,
        name: "Amulet (Casino Coins)",
        offset: None,
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x16,
        name: "Rebellion",
        offset: Some(0x52),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x17,
        name: "Cerberus",
        offset: Some(0x53),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x18,
        name: "Agni and Rudra",
        offset: Some(0x54),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x19,
        name: "Devil Trigger", // Awakened Rebellion
        offset: Some(0x55), // Offset is most likely wrong, but since we use this to give 3 runes, rather than an actual weapon, it should be fine
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1A,
        name: "Nevan",
        offset: Some(0x56),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1B,
        name: "Beowulf",
        offset: Some(0x57),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1C,
        name: "Ebony & Ivory",
        offset: Some(0x58),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1D,
        name: "Shotgun",
        offset: Some(0x59),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1E,
        name: "Artemis",
        offset: Some(0x5A),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1F,
        name: "Spiral",
        offset: Some(0x5B),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x20,
        name: "Dummy",      // Bomb!
        offset: Some(0x5C), // ??
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x21,
        name: "Kalina Ann",
        offset: Some(0x5D),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x22,
        name: "Quicksilver Style",
        offset: Some(0x5E),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x23,
        name: "Doppelganger Style",
        offset: Some(0x5F),
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x24,
        name: "Astronomical Board",
        offset: Some(0x60),
        category: ItemCategory::Key,
        mission: Some(5),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x25,
        name: "Vajura",
        offset: Some(0x61),
        category: ItemCategory::Key,
        mission: Some(5),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x26,
        name: "Remote", // High Roller Card!
        offset: Some(0x62),
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x27,
        name: "Soul of Steel",
        offset: Some(0x63),
        category: ItemCategory::Key,
        mission: Some(5),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x28,
        name: "Essence of Fighting",
        offset: Some(0x64),
        category: ItemCategory::Key,
        mission: Some(6),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x29,
        name: "Essence of Technique",
        offset: Some(0x65),
        category: ItemCategory::Key,
        mission: Some(6),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2A,
        name: "Essence of Intelligence",
        offset: Some(0x66),
        category: ItemCategory::Key,
        mission: Some(6),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2B,
        name: "Orihalcon Fragment",
        offset: Some(0x67),
        category: ItemCategory::Key,
        mission: Some(7),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2C,
        name: "Siren's Shriek",
        offset: Some(0x68),
        category: ItemCategory::Key,
        mission: Some(7),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2D,
        name: "Crystal Skull",
        offset: Some(0x69),
        category: ItemCategory::Key,
        mission: Some(7),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2E,
        name: "Ignis Fatuus",
        offset: Some(0x6A),
        category: ItemCategory::Key,
        mission: Some(8),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2F,
        name: "Ambrosia",
        offset: Some(0x6B),
        category: ItemCategory::Key,
        mission: Some(9),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x30,
        name: "Stone Mask",
        offset: Some(0x6C),
        category: ItemCategory::Key,
        mission: Some(10),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x31,
        name: "Neo Generator",
        offset: Some(0x6D),
        category: ItemCategory::Key,
        mission: Some(10),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x32,
        name: "Haywire Neo Generator",
        offset: Some(0x6E),
        category: ItemCategory::Key,
        mission: Some(12),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x33,
        name: "Full Orihalcon",
        offset: Some(0x6F),
        category: ItemCategory::Key,
        mission: Some(13),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x34,
        name: "Orihalcon Fragment (Right)",
        offset: Some(0x70),
        category: ItemCategory::Key,
        mission: Some(15),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x35,
        name: "Orihalcon Fragment (Bottom)",
        offset: Some(0x71),
        category: ItemCategory::Key,
        mission: Some(15),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x36,
        name: "Orihalcon Fragment (Left)",
        offset: Some(0x72),
        category: ItemCategory::Key,
        mission: Some(15),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x37,
        name: "Golden Sun",
        offset: Some(0x73),
        category: ItemCategory::Key,
        mission: Some(16),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x38,
        name: "Onyx Moonshard",
        offset: Some(0x74),
        category: ItemCategory::Key,
        mission: Some(16),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x39,
        name: "Samsara",
        offset: Some(0x75),
        category: ItemCategory::Key,
        mission: Some(19),
        _max_amount: None,
        _value: None,
    },
];

pub static ITEM_OFFSET_MAP: LazyLock<HashMap<&'static str, u8>> = LazyLock::new(|| {
    ALL_ITEMS
        .iter()
        .filter_map(|item| item.offset.map(|o| (item.name, o)))
        .collect()
});

pub static MISSION_ITEM_MAP: LazyLock<HashMap<u32, Vec<&'static str>>> = LazyLock::new(|| {
    let mut map: HashMap<u32, Vec<&'static str>> = HashMap::new();
    for item in ALL_ITEMS.iter() {
        if let Some(mission) = item.mission {
            map.entry(mission).or_default().push(item.name);
        }
    }
    map
});

pub static ITEM_ID_MAP: LazyLock<HashMap<&'static str, u32>> =
    LazyLock::new(|| ALL_ITEMS.iter().map(|item| (item.name, item.id)).collect());

pub(crate) static ID_ITEM_MAP: LazyLock<HashMap<u32, &'static str>> =
    LazyLock::new(|| ALL_ITEMS.iter().map(|item| (item.id, item.name)).collect());

pub fn get_item_name(item_id: u32) -> &'static str {
    if item_id <= 0x39 {
        ID_ITEM_MAP.get(&item_id).copied().unwrap_or_else(|| {
            log::error!("No item found with id {:#X}", item_id);
            "Unknown"
        })
    } else {
        ID_SKILL_MAP
            .get(&(item_id as usize))
            .copied()
            .unwrap_or_else(|| {
                log::error!("Skill with id of {:#X} was not found", item_id);
                "Unknown"
            })
    }
}

pub fn get_item_id(name: &str) -> Option<u32> {
    match ITEM_ID_MAP.get(name).copied() {
        None => match DATA_PACKAGE.read().unwrap().as_ref() {
            None => None,
            Some(data_package) => data_package
                .dp
                .games
                .get(GAME_NAME)?
                .item_name_to_id
                .get(name)
                .copied()
                .map(|id| id as u32),
        },
        Some(id) => Some(id),
    }
}
pub fn get_items_by_category(category: ItemCategory) -> Vec<&'static str> {
    ALL_ITEMS
        .iter()
        .filter(|item| item.category == category)
        .map(|item| item.name)
        .collect()
}

pub static EVENT_TABLES: LazyLock<HashMap<u32, Vec<EventTable>>> = LazyLock::new(|| {
    HashMap::from([
        (
            3,
            vec![
                EventTable {
                    _mission: 3,
                    location: "Mission #3 - Shotgun",
                    events: vec![
                        Event {
                            event_type: EventCode::Check,
                            offset: 0x450,
                        },
                        Event {
                            event_type: EventCode::Check,
                            offset: 0x6A4,
                        },
                        Event {
                            event_type: EventCode::Give,
                            offset: 0x6DC,
                        },
                        Event {
                            event_type: EventCode::Check,
                            offset: 0x72C,
                        },
                        Event {
                            event_type: EventCode::Give,
                            offset: 0x77C,
                        },
                    ],
                },
                EventTable {
                    _mission: 3,
                    location: "Mission #3 - Cerberus",
                    events: vec![
                        Event {
                            event_type: EventCode::Check,
                            offset: 0xEE4,
                        },
                        Event {
                            event_type: EventCode::Give,
                            offset: 0xEFC,
                        },
                    ],
                },
            ],
        ),
        (
            4,
            vec![EventTable {
                _mission: 4,
                location: "Mission #4 - Astronomical Board",
                events: vec![Event {
                    event_type: EventCode::End,
                    offset: 0x8D4,
                }],
            }],
        ),
        (
            5,
            vec![EventTable {
                _mission: 5,
                location: "Mission #5 - Agni and Rudra",
                events: vec![
                    Event {
                        event_type: EventCode::Check,
                        offset: 0x186C,
                    },
                    Event {
                        event_type: EventCode::Give,
                        offset: 0x1884,
                    },
                ],
            }],
        ),
        (
            6,
            vec![EventTable {
                _mission: 6,
                location: "Mission #6 - Artemis",
                events: vec![
                    Event {
                        event_type: EventCode::Give,
                        offset: 0x13D0,
                    },
                    Event {
                        event_type: EventCode::Check,
                        offset: 0x13C0,
                    },
                ],
            }],
        ),
        (
            9,
            vec![
                EventTable {
                    _mission: 9,
                    location: "Mission #9 - Nevan",
                    events: vec![
                        Event {
                            event_type: EventCode::Check,
                            offset: 0xD4C,
                        },
                        Event {
                            event_type: EventCode::Give,
                            offset: 0xD64,
                        },
                    ],
                },
                EventTable {
                    _mission: 9,
                    location: "Mission #9 - Spiral",
                    events: vec![
                        Event {
                            event_type: EventCode::Check,
                            offset: 0x624,
                        },
                        Event {
                            event_type: EventCode::Give,
                            offset: 0x76C,
                        },
                    ],
                },
            ],
        ),
        (
            12,
            vec![
                EventTable {
                    _mission: 12,
                    location: "Mission #12 - Quicksilver",
                    events: vec![
                        Event {
                            event_type: EventCode::Check,
                            offset: 0x175C,
                        },
                        Event {
                            event_type: EventCode::Give,
                            offset: 0x1774,
                        },
                    ],
                },
                EventTable {
                    _mission: 12,
                    location: "Mission #12 - Haywire Neo Generator",
                    events: vec![Event {
                        event_type: EventCode::Give,
                        offset: 0x130,
                    }],
                },
            ],
        ),
        (
            14,
            vec![EventTable {
                _mission: 14,
                location: "Mission #14 - Beowulf",
                events: vec![
                    Event {
                        event_type: EventCode::Check,
                        offset: 0x94,
                    },
                    Event {
                        event_type: EventCode::Give,
                        offset: 0x15C,
                    },
                ],
            }],
        ),
        (
            16,
            vec![EventTable {
                _mission: 16,
                location: "Mission #16 - Kalina Ann",
                events: vec![
                    Event {
                        event_type: EventCode::Check,
                        offset: 0x1360,
                    },
                    Event {
                        event_type: EventCode::Give,
                        offset: 0x1378,
                    },
                ],
            }],
        ),
        (
            17,
            vec![EventTable {
                _mission: 17,
                location: "Mission #17 - Doppelganger",
                events: vec![
                    Event {
                        event_type: EventCode::Check,
                        offset: 0xA98,
                    },
                    Event {
                        event_type: EventCode::Give,
                        offset: 0xAB0,
                    },
                ],
            }],
        ),
    ])
});

#[derive(PartialEq)]
pub enum EventCode {
    /// Give the provided item (5c 02)
    Give,
    /// Check to see if the player has the specified item in inventory (14 01)
    Check,
    /// End mission if player has item in inventory - (15 01) - Might be wrong/not fully accurate
    End,
}

pub struct Event {
    pub event_type: EventCode,
    pub offset: usize,
}

pub(crate) struct EventTable {
    pub _mission: i32,
    pub location: &'static str,
    pub events: Vec<Event>,
}

#[derive(Debug)]
pub struct ItemEntry {
    // Represents an item on the ground
    pub offset: usize,     // Offset for the item table
    pub room_number: i32,  // Room number
    pub item_id: u32,      // Default Item ID
    pub mission: u32,      // Mission Number
    pub adjudicator: bool, // Adjudicator
    pub coordinates: Coordinates,
}

#[derive(Copy, Clone, strum_macros::Display, strum_macros::FromRepr)]
#[allow(dead_code)]
pub(crate) enum Difficulty {
    Easy = 0,
    Normal = 1,
    Hard = 2,
    #[strum(to_string = "Very Hard")]
    VeryHard = 3,
    #[strum(to_string = "Dante Must Die")]
    DanteMustDie = 4,
    // Swap to boolean check for oneHitKill?
    #[strum(to_string = "Heaven or Hell")]
    HeavenOrHell = 5,
}

//noinspection RsExternalLinter
#[derive(Copy, Clone, strum_macros::Display, strum_macros::FromRepr)]
pub(crate) enum Rank {
    D = 0,
    C = 1,
    B = 2,
    A = 3,
    S = 4,
    SS = 5,
    #[allow(clippy::upper_case_acronyms)]
    SSS = 6,
    ShouldNotBeHere = 7, // DO NOT WANT TO BE HERE
}

pub static GUN_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    Vec::from([
        "Ebony & Ivory",
        "Shotgun",
        "Artemis",
        "Spiral",
        "Kalina Ann",
    ])
});
pub static MELEE_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    Vec::from([
        "Rebellion",
        "Cerberus",
        "Agni and Rudra",
        "Nevan",
        "Beowulf",
    ])
});

pub fn get_weapon_id(weapon: &str) -> u8 {
    match weapon {
        "Rebellion" => 0,
        "Cerberus" => 1,
        "Agni and Rudra" => 2,
        "Nevan" => 3,
        "Beowulf" => 4,
        "Ebony & Ivory" => 5,
        "Shotgun" => 6,
        "Artemis" => 7,
        "Spiral" => 8,
        "Kalina Ann" => 9,
        _ => 0xFF,
    }
}

pub const EMPTY_COORDINATES: Coordinates = Coordinates { x: 0, y: 0, z: 0 };

#[derive(Clone, Copy, Debug)]
pub struct Coordinates {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) z: u32,
}

impl Coordinates {
    pub fn has_coords(&self) -> bool {
        self.x > 0
    }
}

impl PartialEq for Coordinates {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}

#[derive(Copy, Clone, strum_macros::Display, strum_macros::FromRepr)]
pub(crate) enum Character {
    // Values from DDMK, only care about Dante or Vergil though
    Dante,
    _Bob,
    _Lady,
    Vergil,
}
