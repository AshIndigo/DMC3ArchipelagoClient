use crate::game_manager::ArchipelagoData;
use crate::{game_manager, utilities};
use std::collections::HashMap;
use std::ops::BitOrAssign;
use std::ptr::{read_unaligned, write_unaligned};

use crate::constants::Character;
use std::sync::LazyLock;

struct SkillData {
    id: usize,
    index: usize,
    flag: u32,
}

pub static ID_SKILL_MAP: LazyLock<HashMap<usize, &'static str>> = LazyLock::new(|| {
    let mut map: HashMap<usize, &'static str> = SKILLS_MAP
        .iter()
        .map(|(name, data)| (data.id, *name))
        .collect();

    map.extend(HashMap::from([
        (0x53, "Ebony & Ivory Progressive Upgrade"),
        (0x54, "Shotgun Progressive Upgrade"),
        (0x55, "Artemis Progressive Upgrade"),
        (0x56, "Spiral Progressive Upgrade"),
        (0x57, "Kalina Ann Progressive Upgrade"),
        (0x73, "Summoned Swords Progressive Upgrade"),
        (0x74, "Spiral Swords"),
    ]));
    map.extend(HashMap::from([
        (0x60, "Progressive Trickster"),
        (0x61, "Progressive Swordmaster"),
        (0x62, "Progressive Gunslinger"),
        (0x63, "Progressive Royalguard"),
        (0x75, "Progressive Darkslayer"),
    ]));
    map
});

static SKILLS_MAP: LazyLock<HashMap<&str, SkillData>> = LazyLock::new(|| {
    HashMap::from([
        (
            "Rebellion - Stinger Level 1",
            SkillData {
                id: 0x40,
                index: 0,
                flag: 0x80,
            },
        ),
        (
            "Rebellion - Stinger Level 2",
            SkillData {
                id: 0x41,
                index: 0,
                flag: 0x100,
            },
        ),
        (
            "Rebellion - Drive",
            SkillData {
                id: 0x42,
                index: 0,
                flag: 0x2000,
            },
        ),
        (
            "Rebellion - Air Hike",
            SkillData {
                id: 0x43,
                index: 6,
                flag: 0x40000,
            },
        ),
        (
            "Cerberus - Revolver Level 2",
            SkillData {
                id: 0x44,
                index: 1,
                flag: 0x40,
            },
        ),
        (
            "Cerberus - Windmill",
            SkillData {
                id: 0x45,
                index: 0,
                flag: 0x20,
            },
        ),
        (
            "Agni and Rudra - Jet Stream Level 2",
            SkillData {
                id: 0x46,
                index: 1,
                flag: 0x4000000,
            },
        ),
        (
            "Agni and Rudra - Jet Stream Level 3",
            SkillData {
                id: 0x47,
                index: 1,
                flag: 0x8000000,
            },
        ),
        (
            "Agni and Rudra - Whirlwind",
            SkillData {
                id: 0x48,
                index: 1,
                flag: 0x40000000,
            },
        ),
        (
            "Agni and Rudra - Air Hike",
            SkillData {
                id: 0x49,
                index: 6,
                flag: 0x80000,
            },
        ),
        (
            "Nevan - Reverb Shock",
            SkillData {
                id: 0x4A,
                index: 2,
                flag: 0x400000,
            },
        ),
        (
            "Nevan - Reverb Shock Level 2",
            SkillData {
                id: 0x4B,
                index: 2,
                flag: 0x800000,
            },
        ),
        (
            "Nevan - Bat Rift Level 2",
            SkillData {
                id: 0x4C,
                index: 2,
                flag: 0x200000,
            },
        ),
        (
            "Nevan - Air Raid",
            SkillData {
                id: 0x4D,
                index: 3,
                flag: 4,
            },
        ),
        (
            "Nevan - Volume Up",
            SkillData {
                id: 0x4E,
                index: 3,
                flag: 2,
            },
        ),
        (
            "Beowulf - Straight Level 2",
            SkillData {
                id: 0x4F,
                index: 3,
                flag: 0x2000000,
            },
        ),
        (
            "Beowulf - Beast Uppercut",
            SkillData {
                id: 0x50,
                index: 3,
                flag: 0x200000,
            },
        ),
        (
            "Beowulf - Rising Dragon",
            SkillData {
                id: 0x51,
                index: 3,
                flag: 0x400000,
            },
        ),
        (
            "Beowulf - Air Hike",
            SkillData {
                id: 0x52,
                index: 6,
                flag: 0x100000,
            },
        ),
        (
            "Yamato - Rapid Slash Level 1",
            SkillData {
                id: 0x76,
                index: 0,
                flag: 0x10,
            },
        ),
        (
            "Yamato - Rapid Slash Level 2",
            SkillData {
                id: 0x77,
                index: 0,
                flag: 0x20,
            },
        ),
        (
            "Yamato - Judgement Cut Level 1",
            SkillData {
                id: 0x78,
                index: 0,
                flag: 0x200,
            },
        ),
        (
            "Yamato - Judgement Cut Level 2",
            SkillData {
                id: 0x79,
                index: 0,
                flag: 0x400,
            },
        ),
        (
            "Beowulf - Starfall Level 2",
            SkillData {
                id: 0x7A,
                index: 0,
                flag: 0x800000,
            },
        ),
        (
            "Beowulf - Rising Sun",
            SkillData {
                id: 0x7B,
                index: 0,
                flag: 0x2000000,
            },
        ),
        (
            "Beowulf - Lunar Phase Level 2",
            SkillData {
                id: 0x7C,
                index: 0,
                flag: 0x4000000,
            },
        ),
        (
            "Force Edge - Helm Breaker Level 2",
            SkillData {
                id: 0x7D,
                index: 1,
                flag: 0x4,
            },
        ),
        (
            "Force Edge - Stinger Level 1",
            SkillData {
                id: 0x7E,
                index: 1,
                flag: 0x40,
            },
        ),
        (
            "Force Edge - Stinger Level 2",
            SkillData {
                id: 0x7F,
                index: 1,
                flag: 0x80,
            },
        ),
        (
            "Force Edge - Round Trip",
            SkillData {
                id: 0x80,
                index: 1,
                flag: 0x100,
            },
        ),
        // Technically guns, but DMC3 can be weird
        (
            "Summoned Swords Level 2",
            SkillData {
                id: 0x73,
                index: 1,
                flag: 0x40000,
            },
        ),
        (
            "Summoned Swords Level 3",
            SkillData {
                id: 0x74,
                index: 1,
                flag: 0xC0000,
            },
        ),
        (
            "Spiral Swords",
            SkillData {
                id: 0x75,
                index: 1,
                flag: 0x200000,
            },
        ),
    ])
});
static DEFAULT_SKILLS_DANTE: [u32; 8] = [
    // I should see what else this lets me control...
    0xFFFF5E7F, 0xA7FFAF5F, 0xAF1FFFF3, 0xCB9FFFF9, 0xFBFBFFFE, 0xFFFFEFFD, 0xFFE3FEFF, 0xFFFFFFFF,
];

static DEFAULT_SKILLS_VERGIL: [u32; 8] = [
    // I should see what else this lets me control...
    0xF4FFF9CF, 0xFFC7FE37, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
];

pub(crate) fn reset_expertise() {
    game_manager::with_session(|s| {
        s.expertise = match Character::from_repr(s.char as usize).unwrap_or_default() {
            Character::Dante => DEFAULT_SKILLS_DANTE,
            Character::Vergil => DEFAULT_SKILLS_VERGIL,
            _ => DEFAULT_SKILLS_DANTE,
        };
        if let Some(char_data_ptr) = utilities::get_active_char_address() {
            unsafe {
                write_unaligned(
                    (char_data_ptr + EXPERTISE_OFFSET) as *mut [u32; 8],
                    s.expertise,
                )
            }
        }
    })
    .expect("Unable to reset expertise");
}

const EXPERTISE_OFFSET: usize = 0x3FEC; // 0x400c

fn give_skill(skill_id: &usize) {
    // This works, might not update files? need to double-check
    let data = SKILLS_MAP.get(ID_SKILL_MAP.get(skill_id).unwrap()).unwrap();
    game_manager::with_session(|s| {
        s.expertise[data.index].bitor_assign(data.flag);
    })
    .expect("Unable to give skill");

    if let Some(char_data_ptr) = utilities::get_active_char_address() {
        unsafe {
            let mut active_expertise =
                read_unaligned((char_data_ptr + EXPERTISE_OFFSET) as *mut [u32; 8]);
            active_expertise[data.index].bitor_assign(data.flag);
            write_unaligned(
                (char_data_ptr + EXPERTISE_OFFSET) as *mut [u32; 8],
                active_expertise,
            )
        }
    }
}

pub(crate) fn set_skills(data: &ArchipelagoData) {
    // I kinda don't like this tbh, but oh well, shouldn't really be an issue.
    reset_expertise();
    for skill in data.skills.iter() {
        give_skill(skill);
    }
}

// Certain skills have two levels they can gain
pub(crate) fn add_skill(id: usize, data: &mut ArchipelagoData) {
    match id {
        // Rebellion
        0x40 => data.add_stinger_level(),
        0x46 => data.add_jet_stream_level(),
        0x4A => data.add_reverb_level(),
        0x50 => data.add_beowulf_level(),
        // Vergil Stuff
        0x76 => data.add_rapid_slash_level(),
        0x78 => data.add_judgement_cut_level(),
        // Force Edge
        0x7E => data.add_stinger_level(),
        _ => {}
    }

    let skill_id = match id {
        0x40 => match data.stinger_level {
            1 => 0x40,
            2 => 0x41,
            _ => unreachable!(),
        },
        0x46 => match data.jet_stream_level {
            1 => 0x46,
            2 => 0x47,
            _ => unreachable!(),
        },
        0x4A => match data.reverb_level {
            1 => 0x4A,
            2 => 0x4B,
            _ => unreachable!(),
        },
        0x50 => match data.beowulf_level {
            1 => 0x50,
            2 => 0x51,
            _ => unreachable!(),
        },
        0x76 => match data.rapid_slash_level {
            1 => 0x76,
            2 => 0x77,
            _ => unreachable!(),
        },
        0x78 => match data.judgement_cut_level {
            1 => 0x79,
            2 => 0x7A,
            _ => unreachable!(),
        },
        _ => id,
    };
    data.add_skill(skill_id);
}
