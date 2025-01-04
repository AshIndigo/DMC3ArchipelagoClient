use once_cell::sync::OnceCell;
use std::collections::HashMap;

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
        0x26 => "High Roller Card",
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
        _ => "Undefined Item",
    }
}

pub static EVENT_TABLES: OnceCell<HashMap<i32, Vec<EventTable>>> = OnceCell::new();

pub fn set_event_tables() -> HashMap<i32, Vec<EventTable>> {
    let mut tables = HashMap::new();
    tables.insert(
        3,
        vec![EventTable {
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
                }
            ],
        }],
    );
    tables
}

#[derive(PartialEq)]
pub enum EventCode {
    GIVE,
    CHECK,
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
