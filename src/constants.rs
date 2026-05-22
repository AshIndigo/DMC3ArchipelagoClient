use crate::skill_manager::ID_SKILL_MAP;
use bimap::BiMap;
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::sync::LazyLock;

pub(crate) static DUMMY_ID: LazyLock<u32> =
    LazyLock::new(|| *ITEM_MAP.get_by_left("Dummy").unwrap());

pub(crate) static REMOTE_ID: LazyLock<u32> =
    LazyLock::new(|| *ITEM_MAP.get_by_left("Remote").unwrap());
pub const GAME_NAME: &str = "Devil May Cry 3";

// DMC3 Offsets+Functions - Offsets are from 2022 DDMK's version

pub const ITEM_MODE_TABLE: usize = 0x1B4534; // This is actually a constant, we like this one

pub const ONE_ORB: f32 = 1000.0; // One Blue/Purple orb is worth 1000 "points"
pub const BASE_HP: f32 = 6.0 * ONE_ORB;
pub const MAX_HP: f32 = 20000.0;
pub const MAX_MAGIC: f32 = 10000.0;

pub struct Item {
    pub id: u32,
    pub name: &'static str,
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
        category: ItemCategory::RedOrb,
        mission: None,
        _max_amount: Some(999999), // Is what fits on screen, could theoretically go up to MAX_INT
        _value: Some(1),
    },
    Item {
        id: 0x01,
        name: "Red Orb - 5",
        category: ItemCategory::RedOrb,
        mission: None,
        _max_amount: Some(999999),
        _value: Some(5),
    },
    Item {
        id: 0x02,
        name: "Red Orb - 20",
        category: ItemCategory::RedOrb,
        mission: None,
        _max_amount: Some(999999),
        _value: Some(1),
    },
    Item {
        id: 0x03,
        name: "Red Orb - 100",
        category: ItemCategory::RedOrb,
        mission: None,
        _max_amount: Some(999999),
        _value: Some(100),
    },
    Item {
        id: 0x04,
        name: "Red Orb - 1000",
        category: ItemCategory::RedOrb,
        mission: None,
        _max_amount: Some(999999),
        _value: Some(1000),
    },
    Item {
        id: 0x05,
        name: "Gold Orb",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: Some(3),
        _value: None,
    },
    Item {
        id: 0x06,
        name: "Yellow Orb",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: Some(99),
        _value: None,
    },
    Item {
        id: 0x07,
        name: "Blue Orb",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x08,
        name: "Purple Orb",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x09,
        name: "Blue Orb Fragment",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: Some(4),
        _value: None,
    },
    Item {
        id: 0x0A,
        name: "Green Orb - Small",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x0B,
        name: "Green Orb - Medium",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x0C,
        name: "Green Orb - Large",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x0D,
        name: "Unknown D",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x0E,
        name: "Unknown E",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x0F,
        name: "Unknown F",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x10,
        name: "Vital Star L",
        category: ItemCategory::Consumable,
        mission: None,
        _max_amount: Some(30),
        _value: None,
    },
    Item {
        id: 0x11,
        name: "Vital Star S",
        category: ItemCategory::Consumable,
        mission: None,
        _max_amount: Some(30),
        _value: None,
    },
    Item {
        id: 0x12,
        name: "Devil Star",
        category: ItemCategory::Consumable,
        mission: None,
        _max_amount: Some(10),
        _value: None,
    },
    Item {
        id: 0x13,
        name: "Holy Water",
        category: ItemCategory::Consumable,
        mission: None,
        _max_amount: Some(30),
        _value: None,
    },
    Item {
        id: 0x14,
        name: "Scent of Fear", // Scent of Fear test item, old dummy

        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x15,
        name: "Amulet (Casino Coins)",
        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x16,
        name: "Rebellion",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x17,
        name: "Cerberus",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x18,
        name: "Agni and Rudra",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x19,
        name: "Devil Trigger", // Awakened Rebellion

        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1A,
        name: "Nevan",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1B,
        name: "Beowulf",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1C,
        name: "Ebony & Ivory",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1D,
        name: "Shotgun",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1E,
        name: "Artemis",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x1F,
        name: "Spiral",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x20,
        name: "Dummy", // Bomb!

        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x21,
        name: "Kalina Ann",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x22,
        name: "Quicksilver Style",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x23,
        name: "Doppelganger Style",
        category: ItemCategory::Weapon,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x24,
        name: "Astronomical Board",
        category: ItemCategory::Key,
        mission: Some(5),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x25,
        name: "Vajura",
        category: ItemCategory::Key,
        mission: Some(5),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x26,
        name: "Remote", // High Roller Card!

        category: ItemCategory::Misc,
        mission: None,
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x27,
        name: "Soul of Steel",
        category: ItemCategory::Key,
        mission: Some(5),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x28,
        name: "Essence of Fighting",
        category: ItemCategory::Key,
        mission: Some(6),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x29,
        name: "Essence of Technique",
        category: ItemCategory::Key,
        mission: Some(6),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2A,
        name: "Essence of Intelligence",
        category: ItemCategory::Key,
        mission: Some(6),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2B,
        name: "Orihalcon Fragment",
        category: ItemCategory::Key,
        mission: Some(7),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2C,
        name: "Siren's Shriek",
        category: ItemCategory::Key,
        mission: Some(7),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2D,
        name: "Crystal Skull",
        category: ItemCategory::Key,
        mission: Some(7),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2E,
        name: "Ignis Fatuus",
        category: ItemCategory::Key,
        mission: Some(8),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x2F,
        name: "Ambrosia",
        category: ItemCategory::Key,
        mission: Some(9),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x30,
        name: "Stone Mask",
        category: ItemCategory::Key,
        mission: Some(10),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x31,
        name: "Neo Generator",
        category: ItemCategory::Key,
        mission: Some(10),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x32,
        name: "Haywire Neo Generator",
        category: ItemCategory::Key,
        mission: Some(12),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x33,
        name: "Full Orihalcon",
        category: ItemCategory::Key,
        mission: Some(13),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x34,
        name: "Orihalcon Fragment (Right)",
        category: ItemCategory::Key,
        mission: Some(15),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x35,
        name: "Orihalcon Fragment (Bottom)",
        category: ItemCategory::Key,
        mission: Some(15),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x36,
        name: "Orihalcon Fragment (Left)",
        category: ItemCategory::Key,
        mission: Some(15),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x37,
        name: "Golden Sun",
        category: ItemCategory::Key,
        mission: Some(16),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x38,
        name: "Onyx Moonshard",
        category: ItemCategory::Key,
        mission: Some(16),
        _max_amount: None,
        _value: None,
    },
    Item {
        id: 0x39,
        name: "Samsara",
        category: ItemCategory::Key,
        mission: Some(19),
        _max_amount: None,
        _value: None,
    },
];

pub static MISSION_ITEM_MAP: LazyLock<HashMap<u32, Vec<&'static str>>> = LazyLock::new(|| {
    let mut map: HashMap<u32, Vec<&'static str>> = HashMap::new();
    for item in ALL_ITEMS.iter() {
        if let Some(mission) = item.mission {
            map.entry(mission).or_default().push(item.name);
        }
    }
    map
});

pub static ITEM_MAP: LazyLock<BiMap<&'static str, u32>> =
    LazyLock::new(|| ALL_ITEMS.iter().map(|item| (item.name, item.id)).collect());

pub fn get_item_name(item_id: u32) -> &'static str {
    if item_id <= 0x39 {
        ITEM_MAP.get_by_right(&item_id).copied().unwrap_or_else(|| {
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
    pub room_number: i32,  // Room number
    pub item_id: u32,      // Default Item ID
    pub mission: u32,      // Mission Number
    pub adjudicator: bool, // Adjudicator
    pub coordinates: Coordinates,
}

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Deserialize,
    Serialize,
    PartialEq,
    PartialOrd,
    strum_macros::Display,
    strum_macros::FromRepr,
    strum_macros::EnumIter,
    strum_macros::IntoStaticStr,
)]
pub enum Difficulty {
    #[default]
    Easy = 0,
    Normal = 1,
    Hard = 2,
    #[strum(to_string = "Very Hard")]
    #[serde(rename = "Very Hard")]
    VeryHard = 3,
    #[strum(to_string = "Dante Must Die")]
    #[serde(rename = "Dante Must Die")]
    // Need to deserialize this properly...
    DanteMustDie = 4,
    // Swap to boolean check for oneHitKill?
    #[strum(to_string = "Heaven or Hell")]
    #[serde(rename = "Heaven or Hell")]
    HeavenOrHell = 5,
}

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Deserialize,
    Serialize,
    PartialEq,
    PartialOrd,
    strum_macros::Display,
    strum_macros::FromRepr,
)]
pub enum Rank {
    #[default]
    D = 0,
    C = 1,
    B = 2,
    A = 3,
    S = 4,
    SS = 5,
    #[allow(clippy::upper_case_acronyms)]
    SSS = 6,
    //ShouldNotBeHere = 7, // DO NOT WANT TO BE HERE
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

// These two are stupid.
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
        "Quicksilver Style" => 10,
        "Doppelganger Style" => 11,
        "None" => 0xFF,
        _ => 0xFF,
    }
}

pub fn get_unlocked_weapon_id(weapon: &str) -> u8 {
    match weapon {
        "Rebellion" => 0,
        "Cerberus" => 1,
        "Agni and Rudra" => 2,
        "Nevan" => 4,
        "Beowulf" => 5,
        "Ebony & Ivory" => 6,
        "Shotgun" => 7,
        "Artemis" => 8,
        "Spiral" => 9,
        "Kalina Ann" => 11,
        "Quicksilver Style" => 12,
        "Doppelganger Style" => 13,
        "None" => 0xFF,
        _ => 0xFF,
    }
}

pub const EMPTY_COORDINATES: Coordinates = Coordinates { x: 0, y: 0, z: 0 };

#[derive(Clone, Copy, Debug, PartialEq)]
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

#[derive(Copy, Clone, strum_macros::Display, strum_macros::FromRepr)]
pub(crate) enum Character {
    // Values from DDMK, only care about Dante or Vergil though
    Dante,
    _Bob,
    _Lady,
    Vergil,
}

#[derive(
    Copy, Clone, strum_macros::Display, strum_macros::FromRepr, strum_macros::IntoStaticStr,
)]
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

    pub fn get_internal_order_index(&self) -> usize {
        match &self {
            Style::Trickster => 2,
            Style::Swordmaster => 0,
            Style::Gunslinger => 1,
            Style::Royalguard => 3,
        }
    }

    pub const INTERNAL_ORDER: [Style; 4] = [
        Style::Swordmaster,
        Style::Gunslinger,
        Style::Trickster,
        Style::Royalguard,
    ];
}
