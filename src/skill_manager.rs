use crate::game_manager;
use crate::game_manager::ACTIVE_CHAR_DATA;
use crate::utilities::{read_data_from_address, DMC3_ADDRESS};
use std::collections::{HashMap, HashSet};
use std::ptr::{read_unaligned, write_unaligned};

use std::sync::{LazyLock, RwLock};

struct SkillData {
    id: usize,
    index: usize,
    flag: u32,
}

pub static ID_SKILL_MAP: LazyLock<HashMap<usize, &'static str>> = LazyLock::new(|| {
    let mut map: HashMap<usize, &'static str> =
        SKILLS_MAP.iter().map(|item| (item.1.id, *item.0)).collect();

    map.extend(HashMap::from([
        (0x53, "Ebony & Ivory Progressive Upgrade"),
        (0x54, "Shotgun Progressive Upgrade"),
        (0x55, "Artemis Progressive Upgrade"),
        (0x56, "Spiral Progressive Upgrade"),
        (0x57, "Kalina Ann Progressive Upgrade"),
    ]));
    map
});

static SKILLS_MAP: LazyLock<HashMap<&str, SkillData>> = LazyLock::new(|| {
    HashMap::from([
        (
            "Rebellion (Normal) - Stinger Level 1",
            SkillData {
                id: 0x40,
                index: 0,
                flag: 0x80,
            },
        ),
        (
            "Rebellion (Normal) - Stinger Level 2",
            SkillData {
                id: 0x41,
                index: 0,
                flag: 0x100,
            },
        ),
        (
            "Rebellion (Normal) - Drive",
            SkillData {
                id: 0x42,
                index: 0,
                flag: 0x2000,
            },
        ),
        (
            "Rebellion (Normal) - Air Hike",
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
    ])
});
static DEFAULT_SKILLS: [u32; 8] = [
    // I should see what else this lets me control...
    0xFFFF5E7F, 0xA7FFAF5F, 0xAF1FFFF3, 0xCB9FFFF9, 0xFBFBFFFE, 0xFFFFEFFD, 0xFFE3FEFF, 0xFFFFFFFF,
];

pub(crate) fn reset_expertise() {
    game_manager::with_session(|s| {
        s.expertise = DEFAULT_SKILLS;
    })
    .expect("Unable to reset expertise");
}

fn give_skill(skill_name: &'static str) {
    // This works, might not update files? need to double-check
    log::debug!("Giving skill: {}", skill_name);
    let data = SKILLS_MAP.get(&skill_name).unwrap();
    game_manager::with_session(|s| {
        s.expertise[data.index] += data.flag;
    })
    .expect("Unable to give skill");
    let expertise_offset = 0x3FEC;
    let char_data_ptr: usize = read_data_from_address(*DMC3_ADDRESS + ACTIVE_CHAR_DATA);
    if char_data_ptr != 0 {
        unsafe {
            let mut active_expertise =
                read_unaligned((char_data_ptr + expertise_offset) as *mut [u32; 8]);
            active_expertise[data.index] += data.flag;
            write_unaligned(
                (char_data_ptr + expertise_offset) as *mut [u32; 8],
                active_expertise,
            )
        }
    }
}

pub(crate) fn set_skills() {
    // I kinda don't like this tbh, but oh well, shouldn't really be an issue.
    reset_expertise();
    for skill in ACQUIRED_SKILLS.read().unwrap().iter() {
        give_skill(*skill);
    }
}

// Certain skills have two levels they can gain
pub static ACQUIRED_SKILLS: LazyLock<RwLock<HashSet<&'static str>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

pub(crate) fn add_skill(id: usize) {
    match game_manager::ARCHIPELAGO_DATA.write() {
        Ok(mut data) => match id {
            0x40 => {
                data.add_stinger_level();
            }
            0x46 => {
                data.add_jet_stream_level();
            }
            0x4A => {
                data.add_reverb_level();
            }
            _ => {}
        },
        Err(err) => {
            log::error!("Failed to get ArchipelagoData: {}", err);
        }
    }
    match game_manager::ARCHIPELAGO_DATA.read() {
        Ok(data) => match (*ACQUIRED_SKILLS).write() {
            Ok(mut skill_list) => {
                let skill_name = match id {
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
                    _ => id,
                };
                skill_list.insert(ID_SKILL_MAP.get(&skill_name).unwrap());
            }
            Err(err) => {
                log::error!(
                    "Unable to write to internal acquired skills list: {:?}",
                    err
                );
            }
        },
        Err(err) => {
            log::error!("Failed to get ArchipelagoData: {}", err);
        }
    }

    return;
}
