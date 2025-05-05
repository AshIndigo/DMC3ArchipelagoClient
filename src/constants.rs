use std::collections::HashMap;
use std::sync::LazyLock;
use std::ffi::{c_int, c_longlong};
use std::os::raw::c_short;

// DMC3 Offsets+Functions - Offsets are from 2022 DDMK's version
pub type ItemPickedUpFunc = unsafe extern "C" fn(loc_chk_id: c_longlong, param_2: c_short, item_id: c_int);
pub const ITEM_PICKED_UP_ADDR: usize = 0x1aa6e0;
pub static mut ORIGINAL_ITEMPICKEDUP: Option<ItemPickedUpFunc> = None;

pub type ItemHandlePickupFunc = unsafe extern "C" fn(item_struct: c_longlong);
pub const ITEM_HANDLE_PICKUP_ADDR: usize = 0x1b45a0;
pub static mut ORIGINAL_HANDLE_PICKUP: Option<ItemHandlePickupFunc> = None;

pub type ItemSpawns = unsafe extern "C" fn(loc_chk_id: c_longlong);
pub const ITEM_SPAWNS_ADDR: usize = 0x1b4440;  // 0x1b4480
pub static mut ORIGINAL_ITEM_SPAWNS: Option<ItemSpawns> = None;
pub const INVENTORY_PTR: usize = 0xC90E28 + 0x8;

pub const ADJUDICATOR_ITEM_ID_1: usize = 0x250594;
pub const ADJUDICATOR_ITEM_ID_2: usize = 0x25040d;
pub const ITEM_MODE_TABLE: usize = 0x1B4534;
pub const EVENT_TABLE_ADDR: usize = 0x01A42680; // TODO is this gonna be ok?

pub const STARTING_MELEE: usize = 0x0; // TODO
pub const STARTING_GUN: usize = 0x0; // TODO

pub(crate) const KEY_ITEMS: [&str; 21] = [
    "Astronomical Board",          // 0
    "Vajura",                      // 1
    "Essence of Intelligence",     // 2
    "Essence of Technique",        // 3
    "Essence of Fighting",          // 4
    "Soul of Steel",               // 5
    "Full Orihalcon",              // 6
    "Orihalcon Fragment",          // 7
    "Orihalcon Fragment (Right)",  // 8
    "Orihalcon Fragment (Bottom)", // 9
    "Orihalcon Fragment (Left)",   //10
    "Siren's Shriek",              //11
    "Crystal Skull",               //12
    "Ignis Fatuus",                //13
    "Ambrosia",                    //14
    "Stone Mask",                  //15
    "Neo Generator",               //16
    "Haywire Neo Generator",       //17
    "Golden Sun",                  //18
    "Onyx Moonshard",              //19
    "Samsara",                     //20
];

pub(crate) const CONSUMABLES: [&str; 5] = [
    "Vital Star L",          // 0
    "Vital Star S",          // 1
    "Devil Star",            // 2
    "Holy Water",            // 3
    "Red Orbs" // Remove? 4
];

// What item is used in what mission
pub static ITEM_MISSION_MAP: LazyLock<HashMap<&'static str, i32>> = LazyLock::new(|| {
    HashMap::from([
        (KEY_ITEMS[0], 5),   // Astronomical Board, obtained at the end of M4, used in M5
        (KEY_ITEMS[1], 5),   // Vajura
        (KEY_ITEMS[2], 6),   // Essence of Intelligence
        (KEY_ITEMS[3], 6),   // Essence of Technique
        (KEY_ITEMS[4], 6),   // Essence of Fighting
        (KEY_ITEMS[5], 5),   // Soul of Steel
        (KEY_ITEMS[6], 13),  // Full Orihalcon
        (KEY_ITEMS[7], 7),   // Orihalcon Fragment
        (KEY_ITEMS[8], 15),  // Orihalcon Fragment (Right)
        (KEY_ITEMS[9], 15),  // Orihalcon Fragment (Bottom)
        (KEY_ITEMS[10], 15), // Orihalcon Fragment (Left)
        (KEY_ITEMS[11], 7),  // Siren's Shriek
        (KEY_ITEMS[12], 7),  // Crystal Skull
        (KEY_ITEMS[13], 8),  // Ignis Fatuus
        (KEY_ITEMS[14], 9),  // Ambrosia
        (KEY_ITEMS[15], 10), // Stone Mask
        (KEY_ITEMS[16], 10), // Neo Generator
        (KEY_ITEMS[17], 12), // Haywire Neo Generator
        (KEY_ITEMS[18], 16), // Golden Sun
        (KEY_ITEMS[19], 16), // Onyx Moonshard
        (KEY_ITEMS[20], 19), // Samsara
    ])
});

pub static MISSION_ITEM_MAP: LazyLock<HashMap<i32, Vec<&'static str>>> = LazyLock::new(|| {
    let mut map: HashMap<i32, Vec<&'static str>> = HashMap::new();
    for (&item, &mission) in ITEM_MISSION_MAP.iter() {
        map.entry(mission).or_default().push(item);
    }
    map
});

pub static KEY_ITEM_OFFSETS: LazyLock<HashMap<&'static str, i32>> = LazyLock::new(|| {
    // Unknown: 3 (High roller card most likely)
    HashMap::from([
        (KEY_ITEMS[0], 1),   // Astronomical Board
        (KEY_ITEMS[1], 2),   // Vajura
        (KEY_ITEMS[2], 7),   // Essence of Intelligence
        (KEY_ITEMS[3], 6),   // Essence of Technique
        (KEY_ITEMS[4], 5),   // Essence of Fighting
        (KEY_ITEMS[5], 4),   // Soul of Steel
        (KEY_ITEMS[6], 16),  // Full Orihalcon
        (KEY_ITEMS[7], 8),   // Orihalcon Fragment
        (KEY_ITEMS[8], 17),  // Orihalcon Fragment (Right)
        (KEY_ITEMS[9], 18),  // Orihalcon Fragment (Bottom)
        (KEY_ITEMS[10], 19), // Orihalcon Fragment (Left)
        (KEY_ITEMS[11], 9),  // Siren's Shriek
        (KEY_ITEMS[12], 10),  // Crystal Skull
        (KEY_ITEMS[13], 11),  // Ignis Fatuus
        (KEY_ITEMS[14], 12),  // Ambrosia
        (KEY_ITEMS[15], 13), // Stone Mask
        (KEY_ITEMS[16], 14), // Neo Generator
        (KEY_ITEMS[17], 15), // Haywire Neo Generator
        (KEY_ITEMS[18], 20), // Golden Sun
        (KEY_ITEMS[19], 21), // Onyx Moonshard
        (KEY_ITEMS[20], 22), // Samsara
    ])
});

pub fn get_item_id(item_name: &str) -> Option<u8> {
    match item_name {
        "Red Orb - 1" => Some(0x00),
        "Red Orb - 5" => Some(0x01),
        "Red Orb - 20" => Some(0x02),
        "Red Orb - 100" => Some(0x03),
        "Red Orb - 1000" => Some(0x04),
        "Gold Orb" => Some(0x05),
        "Yellow Orb" => Some(0x06),
        "Blue Orb" => Some(0x07),
        "Purple Orb" => Some(0x08),
        "Blue Orb Fragment" => Some(0x09),
        "Green Orb" => Some(0x0A),
        "Grorb" => Some(0x0B),
        "Big Green Orb" => Some(0x0C),
        "TODO" => Some(0x0D), // Applies to multiple TODO cases
        "Vital Star L" => Some(0x10),
        "Vital Star S" => Some(0x11),
        "Devil Star" => Some(0x12),
        "Holy Water" => Some(0x13),
        "Reb Orb (Fear Test Test)" => Some(0x14),
        "Amulet (Casino Coins)" => Some(0x15),
        "Rebellion (Normal)" => Some(0x16),
        "Cerberus" => Some(0x17),
        "Agni and Rudra" => Some(0x18),
        "Rebellion (Awakened)" => Some(0x19),
        "Nevan" => Some(0x1A),
        "Beowulf" => Some(0x1B),
        "Ebony & Ivory" => Some(0x1C),
        "Shotgun" => Some(0x1D),
        "Artemis" => Some(0x1E), //?
        "Spiral" => Some(0x1F),  // ?
        "Red Orb...? (Bomb!)" => Some(0x20),
        "Kalina Ann" => Some(0x21),
        "Quicksilver" => Some(0x22),
        "Dopl Style" => Some(0x23),
        "Astronomical Board" => Some(0x24),
        "Vajura" => Some(0x25),
        "High Roller Card" => Some(0x26),
        "Soul of Steel" => Some(0x27),
        "Essence of Fighting" => Some(0x28),
        "Essence of Technique" => Some(0x29),
        "Essence of Intelligence" => Some(0x2A),
        "Orihalcon Fragment" => Some(0x2B),
        "Siren's Shriek" => Some(0x2C),
        "Crystal Skull" => Some(0x2D),
        "Ignis Fatuus" => Some(0x2E),
        "Ambrosia" => Some(0x2F),
        "Stone Mask" => Some(0x30),
        "Neo Generator" => Some(0x31),
        "Haywire Neo Generator" => Some(0x32),
        "Full Orihalcon" => Some(0x33),
        "Orihalcon Fragment (Right)" => Some(0x34),
        "Orihalcon Fragment (Bottom)" => Some(0x35),
        "Orihalcon Fragment (Left)" => Some(0x36),
        "Golden Sun" => Some(0x37),
        "Onyx Moonshard" => Some(0x38),
        "Samsara" => Some(0x39),
        "Remote" => Some(0x26),
        _ => None, // Handle undefined items
    }
}
pub fn get_item(item_id: u64) -> &'static str {
    // TODO Update the strings in this
    match item_id {
        0x00 => "Red Orb - 1",
        0x01 => "Red Orb - 5",
        0x02 => "Red Orb - 20",
        0x03 => "Red Orb - 100",
        0x04 => "Red Orb - 1000",
        0x05 => "Gold Orb",
        0x06 => "Yellow Orb",
        0x07 => "Blue Orb (No Work)",
        0x08 => "Purple Orb (No Work)",
        0x09 => "Blue Orb Frag",
        0x0A => "Green Orb",
        0x0B => "Grorb",
        0x0C => "Big Green Orb",
        0x0D => "TODO",
        0x0E => "TODO",
        0x0F => "TODO",
        0x10 => "Vital Star L",
        0x11 => "Vital Star S",
        0x12 => "Devil Star",
        0x13 => "Holy Water",
        0x14 => "Reb Orb (Fear Test Test)",
        0x15 => "Amulet (Casino Coins)",
        0x16 => "Rebellion (Normal)",
        0x17 => "Cerberus",
        0x18 => "Agni?",
        0x19 => "Rebellion Awakened",
        0x1A => "Nevan",
        0x1B => "Beowulf",
        0x1C => "E&I",
        0x1D => "Shotgun",
        0x1E => "Artemis(?)",
        0x1F => "Spiral(?)",
        0x20 => "Red Orb...? (Bomb!)",
        0x21 => "Kalina Ann",
        0x22 => "Quicksilver",
        0x23 => "Dopl Style",
        0x24 => "Astro Board",
        0x25 => "Vajura",
        //0x26 => "High Roller Card",
        0x27 => "Soul of Steel",
        0x28 => "Essence of Fighting",
        0x29 => "Essence of Technique",
        0x2A => "Essence of Intelligence",
        0x2B => "Orihalcon Frag",
        0x2C => "TODO",
        0x2D => "TODO",
        0x2E => "TODO",
        0x2F => "TODO",
        0x30 => "Stone Mask",
        0x31 => "Neo Gen",
        0x32 => "Haywire Neo",
        0x33 => "Full Orihalcon",
        0x34 => "Orihalcon Fragment (Right)",
        0x35 => "Orihalcon Fragment (Bottom)",
        0x36 => "Orihalcon Fragment (Left)",
        0x37 => "Golden Sun",
        0x38 => "Onyx Moonshard",
        0x39 => "Samsara",
        0x26 => "Remote",
        _ => "Undefined Item",
    }
}

pub static EVENT_TABLES: LazyLock<HashMap<i32, Vec<EventTable>>> = LazyLock::new(|| {
    HashMap::from([
        (
            3,
            vec![
                EventTable {
                    mission: 3,
                    location: "Mission #3 - Shotgun".to_string(),
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
                    mission: 3,
                    location: "Mission #3 - Cerberus".to_string(),
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
                mission: 4,
                location: "Mission #4 - Astronomical Board".to_string(),
                events: vec![Event {
                    event_type: EventCode::END,
                    offset: 0x8D4,
                }],
            }],
        ),
        (
            5,
            vec![
                EventTable {
                    mission: 5,
                    location: "Mission #5 - Agni and Rudra".to_string(),
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
                },
                // EventTable {
                //     mission: 5,
                //     location: "Mission #5 - Soul of Steel".to_string(),
                //     events: vec![
                //         Event { event_type: EventCode::CHECK, offset: 0x186C },
                //         Event { event_type: EventCode::GIVE, offset: 0x1884 }
                //     ],
                // }
            ],
        ),
        (6,
         vec![
             EventTable {
                 mission: 6,
                 location: "Mission #6 - Artemis".to_string(),
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
             },
        ]),
    ])
});

/*pub fn set_event_tables() -> HashMap<i32, Vec<EventTable>> {
    let mut tables = HashMap::new(); // TODO FILL OUT
    tables.insert(
        3,
        vec![
            EventTable {
                mission: 3,
                location: "Mission #3 - Shotgun".to_string(),
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
                mission: 3,
                location: "Mission #3 - Cerberus".to_string(),
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
    );
    tables
}
*/
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
    pub mission: i32,
    pub location: String,
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