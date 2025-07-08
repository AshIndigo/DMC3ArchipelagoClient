use std::cmp::PartialEq;
use std::collections::HashMap;
use std::ffi::{c_int, c_longlong};
use std::os::raw::c_short;
use std::sync::{LazyLock, OnceLock};

// DMC3 Offsets+Functions - Offsets are from 2022 DDMK's version
pub const ITEM_PICKED_UP_ADDR: usize = 0x1aa6e0;
pub static ORIGINAL_ITEM_PICKED_UP: OnceLock<
    unsafe extern "C" fn(loc_chk_id: c_longlong, param_2: c_short, item_id: c_int),
> = OnceLock::new();

pub const RESULT_SCREEN_ADDR: usize = 0x2a0850; //Constructor for result screen
pub static ORIGINAL_HANDLE_MISSION_COMPLETE: OnceLock<unsafe extern "C" fn(this: c_longlong)> =
    OnceLock::new(); //, param_2: c_longlong, param_3: c_longlong, param_4: c_longlong)> = OnceLock::new();

pub const RENDER_TEXT_ADDR: usize = 0x2f0440;
pub static ORIGINAL_RENDER_TEXT: OnceLock<
    unsafe extern "C" fn(
        param_1: c_longlong,
        param_2: c_longlong,
        param_3: c_longlong,
        param_4: c_longlong,
    ),
> = OnceLock::new();

pub const ITEM_HANDLE_PICKUP_ADDR: usize = 0x1b45a0;
pub static ORIGINAL_HANDLE_PICKUP: OnceLock<unsafe extern "C" fn(item_struct: c_longlong)> =
    OnceLock::new();

pub const ITEM_SPAWNS_ADDR: usize = 0x1b4440; // 0x1b4480
pub static ORIGINAL_ITEM_SPAWNS: OnceLock<unsafe extern "C" fn(loc_chk_id: c_longlong)> =
    OnceLock::new();

pub const EDIT_EVENT_HOOK_ADDR: usize = 0x1a9bc0;
pub static ORIGINAL_EDIT_EVENT: OnceLock<
    unsafe extern "C" fn(param_1: c_longlong, param_2: c_int, param_3: c_longlong),
> = OnceLock::new();
pub const INVENTORY_PTR: usize = 0xC90E28 + 0x8;
pub const ADJUDICATOR_ITEM_ID_1: usize = 0x250594;
pub const ADJUDICATOR_ITEM_ID_2: usize = 0x25040d;
pub const SECRET_MISSION_ITEM: usize = 0x1a7a4d;
pub const ITEM_MODE_TABLE: usize = 0x1B4534;
pub const EVENT_TABLE_ADDR: usize = 0x01A42680; // TODO is this gonna be ok?

pub const STARTING_MELEE: usize = 0xC8F250 + 0x46; // TODO Think is the "obtained" bool, need the starting weapon inv
pub const STARTING_GUN: usize = 0xC8F250 + 0x4C; // TODO

pub struct Item {
    pub id: u8,
    pub name: &'static str,
    pub offset: Option<u8>, // Inventory offset
    pub category: ItemCategory,
    pub mission: Option<u8>, // Mission the key item is used in, typically the same that it is acquired in
    pub max_amount: Option<i32>, // Max amount of a consumable
    pub _value: Option<i32>, // Value of an orb, used only for red orbs
}

#[derive(PartialEq)]
pub(crate) enum ItemCategory {
    Key,
    Consumable,
    Weapon,
    RedOrb, // Red orbs are special...
    Misc,
}

static ALL_ITEMS: LazyLock<Vec<Item>> = LazyLock::new(|| {
    vec![
        Item {
            id: 0x00,
            name: "Red Orb - 1",
            offset: Some(0x38),
            category: ItemCategory::RedOrb,
            mission: None,
            max_amount: Some(999999), // Is what fits on screen, could theoretically go up to MAX_INT
            _value: Some(1),
        },
        Item {
            id: 0x01,
            name: "Red Orb - 5",
            offset: Some(0x38),
            category: ItemCategory::RedOrb,
            mission: None,
            max_amount: Some(999999),
            _value: Some(5),
        },
        Item {
            id: 0x02,
            name: "Red Orb - 20",
            offset: Some(0x38),
            category: ItemCategory::RedOrb,
            mission: None,
            max_amount: Some(999999),
            _value: Some(1),
        },
        Item {
            id: 0x03,
            name: "Red Orb - 100",
            offset: Some(0x38),
            category: ItemCategory::RedOrb,
            mission: None,
            max_amount: Some(999999),
            _value: Some(100),
        },
        Item {
            id: 0x04,
            name: "Red Orb - 1000",
            offset: Some(0x38),
            category: ItemCategory::RedOrb,
            mission: None,
            max_amount: Some(999999),
            _value: Some(1000),
        },
        Item {
            id: 0x05,
            name: "Gold Orb",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: Some(3),
            _value: None,
        },
        Item {
            id: 0x06,
            name: "Yellow Orb",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: Some(99),
            _value: None,
        },
        Item {
            id: 0x07,
            name: "Blue Orb",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x08,
            name: "Purple Orb",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x09,
            name: "Blue Orb Fragment",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: Some(4),
            _value: None,
        },
        Item {
            id: 0x0A,
            name: "Green Orb - Small",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x0B,
            name: "Green Orb - Medium",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x0C,
            name: "Green Orb - Large",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x0D,
            name: "Unknown D",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x0E,
            name: "Unknown E",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x0F,
            name: "Unknown F",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x10,
            name: "Vital Star L",
            offset: Some(0x4C),
            category: ItemCategory::Consumable,
            mission: None,
            max_amount: Some(30),
            _value: None,
        },
        Item {
            id: 0x11,
            name: "Vital Star S",
            offset: Some(0x4D),
            category: ItemCategory::Consumable,
            mission: None,
            max_amount: Some(30),
            _value: None,
        },
        Item {
            id: 0x12,
            name: "Devil Star",
            offset: Some(0x4E),
            category: ItemCategory::Consumable,
            mission: None,
            max_amount: Some(10),
            _value: None,
        },
        Item {
            id: 0x13,
            name: "Holy Water",
            offset: Some(0x4F),
            category: ItemCategory::Consumable,
            mission: None,
            max_amount: Some(30),
            _value: None,
        },
        Item {
            id: 0x14,
            name: "Dummy", // Scent of Fear test item
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x15,
            name: "Amulet (Casino Coins)",
            offset: None,
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x16,
            name: "Rebellion (Normal)",
            offset: Some(0x52),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x17,
            name: "Cerberus",
            offset: Some(0x53),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x18,
            name: "Agni and Rudra",
            offset: Some(0x54),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x19,
            name: "Rebellion (Awakened)",
            offset: Some(0x52), // TODO, using the same offset as rebellion even if not correct. Need to determine what awakens Rebellion
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x1A,
            name: "Nevan",
            offset: Some(0x56),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x1B,
            name: "Beowulf",
            offset: Some(0x57),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x1C,
            name: "Ebony & Ivory",
            offset: Some(0x58),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x1D,
            name: "Shotgun",
            offset: Some(0x59),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x1E,
            name: "Artemis",
            offset: Some(0x5A),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x1F,
            name: "Spiral",
            offset: Some(0x5B),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x20,
            name: "Dummy", // Bomb!
            offset: Some(0x5C), // ??
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x21,
            name: "Kalina Ann",
            offset: Some(0x5D),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x22,
            name: "Quicksilver Style",
            offset: Some(0x5E),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x23,
            name: "Doppelganger Style",
            offset: Some(0x5F),
            category: ItemCategory::Weapon,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x24,
            name: "Astronomical Board",
            offset: Some(0x60),
            category: ItemCategory::Key,
            mission: Some(5),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x25,
            name: "Vajura",
            offset: Some(0x61),
            category: ItemCategory::Key,
            mission: Some(5),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x26,
            name: "Remote", // High Roller Card!
            offset: Some(0x62),
            category: ItemCategory::Misc,
            mission: None,
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x27,
            name: "Soul of Steel",
            offset: Some(0x63),
            category: ItemCategory::Key,
            mission: Some(5),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x28,
            name: "Essence of Fighting",
            offset: Some(0x64),
            category: ItemCategory::Key,
            mission: Some(6),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x29,
            name: "Essence of Technique",
            offset: Some(0x65),
            category: ItemCategory::Key,
            mission: Some(6),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x2A,
            name: "Essence of Intelligence",
            offset: Some(0x66),
            category: ItemCategory::Key,
            mission: Some(6),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x2B,
            name: "Orihalcon Fragment",
            offset: Some(0x67),
            category: ItemCategory::Key,
            mission: Some(7),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x2C,
            name: "Siren's Shriek",
            offset: Some(0x68),
            category: ItemCategory::Key,
            mission: Some(7),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x2D,
            name: "Crystal Skull",
            offset: Some(0x69),
            category: ItemCategory::Key,
            mission: Some(7),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x2E,
            name: "Ignis Fatuus",
            offset: Some(0x6A),
            category: ItemCategory::Key,
            mission: Some(8),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x2F,
            name: "Ambrosia",
            offset: Some(0x6B),
            category: ItemCategory::Key,
            mission: Some(9),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x30,
            name: "Stone Mask",
            offset: Some(0x6C),
            category: ItemCategory::Key,
            mission: Some(10),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x31,
            name: "Neo Generator",
            offset: Some(0x6D),
            category: ItemCategory::Key,
            mission: Some(10),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x32,
            name: "Haywire Neo Generator",
            offset: Some(0x6E),
            category: ItemCategory::Key,
            mission: Some(12),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x33,
            name: "Full Orihalcon",
            offset: Some(0x6F),
            category: ItemCategory::Key,
            mission: Some(13),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x34,
            name: "Orihalcon Fragment (Right)",
            offset: Some(0x70),
            category: ItemCategory::Key,
            mission: Some(15),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x35,
            name: "Orihalcon Fragment (Bottom)",
            offset: Some(0x71),
            category: ItemCategory::Key,
            mission: Some(15),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x36,
            name: "Orihalcon Fragment (Left)",
            offset: Some(0x72),
            category: ItemCategory::Key,
            mission: Some(15),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x37,
            name: "Golden Sun",
            offset: Some(0x73),
            category: ItemCategory::Key,
            mission: Some(16),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x38,
            name: "Onyx Moonshard",
            offset: Some(0x74),
            category: ItemCategory::Key,
            mission: Some(16),
            max_amount: None,
            _value: None,
        },
        Item {
            id: 0x39,
            name: "Samsara",
            offset: Some(0x75),
            category: ItemCategory::Key,
            mission: Some(19),
            max_amount: None,
            _value: None,
        },
    ]
});

pub static ITEM_OFFSET_MAP: LazyLock<HashMap<&'static str, u8>> = LazyLock::new(|| {
    ALL_ITEMS
        .iter()
        .filter_map(|item| item.offset.map(|o| (item.name, o)))
        .collect()
});

pub static MISSION_ITEM_MAP: LazyLock<HashMap<u8, Vec<&'static str>>> = LazyLock::new(|| {
    let mut map: HashMap<u8, Vec<&'static str>> = HashMap::new();
    for item in ALL_ITEMS.iter() {
        if let Some(mission) = item.mission {
            map.entry(mission).or_default().push(item.name);
        }
    }
    map
});

pub static ITEM_ID_MAP: LazyLock<HashMap<&'static str, u8>> =
    LazyLock::new(|| ALL_ITEMS.iter().map(|item| (item.name, item.id)).collect());

pub static ITEM_MAX_COUNT_MAP: LazyLock<HashMap<&'static str, Option<i32>>> = LazyLock::new(|| {
    ALL_ITEMS
        .iter()
        .map(|item| (item.name, item.max_amount))
        .collect()
});

pub static ID_ITEM_MAP: LazyLock<HashMap<u8, &'static str>> =
    LazyLock::new(|| ALL_ITEMS.iter().map(|item| (item.id, item.name)).collect());
pub static ITEM_MAP: LazyLock<HashMap<u8, &'static Item>> =
    LazyLock::new(|| ALL_ITEMS.iter().map(|item| (item.id, item)).collect());

pub fn get_item_name(item_id: u8) -> &'static str {
    ID_ITEM_MAP.get(&item_id).copied().unwrap_or_else(|| {
        log::error!("No item found with id {}", item_id);
        "Unknown"
    })
}

pub fn get_item_id(name: &str) -> Option<u8> {
    ITEM_ID_MAP.get(name).copied()
}
pub fn get_items_by_category(category: ItemCategory) -> Vec<&'static str> {
    ALL_ITEMS
        .iter()
        .filter(|item| item.category == category)
        .map(|item| item.name)
        .collect()
}

pub static EVENT_TABLES: LazyLock<HashMap<u8, Vec<EventTable>>> = LazyLock::new(|| {
    HashMap::from([
        (
            3,
            vec![
                EventTable {
                    _mission: 3,
                    location: "Mission #3 - Shotgun",
                    events: vec![
                        Event {
                            event_type: EventCode::CHECK,
                            offset: 0x450,
                        },
                        Event {
                            event_type: EventCode::CHECK,
                            offset: 0x6A4,
                        },
                        Event {
                            event_type: EventCode::GIVE,
                            offset: 0x6DC,
                        },
                        Event {
                            event_type: EventCode::CHECK,
                            offset: 0x72C,
                        },
                        Event {
                            event_type: EventCode::GIVE,
                            offset: 0x77C,
                        },
                    ],
                },
                EventTable {
                    _mission: 3,
                    location: "Mission #3 - Cerberus",
                    events: vec![
                        Event {
                            event_type: EventCode::CHECK,
                            offset: 0xEE4,
                        },
                        Event {
                            event_type: EventCode::GIVE,
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
                    event_type: EventCode::END,
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
                        event_type: EventCode::CHECK,
                        offset: 0x186C,
                    },
                    Event {
                        event_type: EventCode::GIVE,
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
                        event_type: EventCode::CHECK,
                        offset: 0x13CC,
                    },
                    Event {
                        event_type: EventCode::GIVE,
                        offset: 0x13D0,
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
                            event_type: EventCode::CHECK,
                            offset: 0xD4C,
                        },
                        Event {
                            event_type: EventCode::GIVE,
                            offset: 0xD64,
                        },
                    ],
                },
                EventTable {
                    _mission: 9,
                    location: "Mission #9 - Spiral",
                    events: vec![
                        Event {
                            event_type: EventCode::CHECK,
                            offset: 0x624,
                        },
                        Event {
                            event_type: EventCode::GIVE,
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
                            event_type: EventCode::CHECK,
                            offset: 0x175C,
                        },
                        Event {
                            event_type: EventCode::GIVE,
                            offset: 0x1774,
                        },
                    ],
                },
                EventTable {
                    _mission: 12,
                    location: "Mission #12 - Haywire Neo Generator",
                    events: vec![Event {
                        event_type: EventCode::GIVE,
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
                        event_type: EventCode::CHECK,
                        offset: 0x94,
                    },
                    Event {
                        event_type: EventCode::GIVE,
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
                        event_type: EventCode::CHECK,
                        offset: 0x1360,
                    },
                    Event {
                        event_type: EventCode::GIVE,
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
                        event_type: EventCode::CHECK,
                        offset: 0xA98,
                    },
                    Event {
                        event_type: EventCode::GIVE,
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
    GIVE,
    /// Check to see if the player has the specified item in inventory (14 01)
    CHECK,
    /// End mission if player has item in inventory - TODO Check again - (15 01)
    END,
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

pub const GAME_NAME: &str = "Devil May Cry 3";

pub(crate) enum Status {
    Disconnected = 0,
    Connected = 1,
    InvalidSlot = 2,
    InvalidGame = 3,
    IncompatibleVersion = 4,
    InvalidPassword = 5,
    InvalidItemHandling = 6,
}

impl From<Status> for isize {
    fn from(value: Status) -> Self {
        match value {
            Status::Disconnected => 0,
            Status::Connected => 1,
            Status::InvalidSlot => 2,
            Status::InvalidGame => 3,
            Status::IncompatibleVersion => 4,
            Status::InvalidPassword => 5,
            Status::InvalidItemHandling => 6,
        }
    }
}

impl From<isize> for Status {
    fn from(value: isize) -> Self {
        match value {
            0 => Status::Disconnected,
            1 => Status::Connected,
            2 => Status::InvalidSlot,
            3 => Status::InvalidGame,
            4 => Status::IncompatibleVersion,
            5 => Status::InvalidPassword,
            6 => Status::InvalidItemHandling,
            _ => Status::Disconnected,
        }
    }
}

pub struct ItemEntry {
    // Represents an item on the ground
    pub offset: usize,     // Offset for the item table
    pub room_number: u16,  // Room number
    pub item_id: u8,       // Default Item ID
    pub mission: u8,       // Mission Number
    pub adjudicator: bool, // Adjudicator
    pub x_coord: u32,
    pub y_coord: u32,
    pub z_coord: u32,
}
