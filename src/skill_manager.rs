use crate::game_manager::ArchipelagoData;
use std::collections::HashMap;
use std::ops::BitOrAssign;

use crate::data::game_structs::{CharacterData, GameData, SessionData};
use bitflags::bitflags;
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
    ]));
    map.extend(HashMap::from([
        (0x60, "Progressive Trickster"),
        (0x61, "Progressive Swordmaster"),
        (0x62, "Progressive Gunslinger"),
        (0x63, "Progressive Royalguard"),
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
                flag: Exp0::Rebellion_Stinger_1.bits(),
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
                index: 1,
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

/*
   Notes:
       - Free Ride, Taunt, Jump/Wall Jump are all built in
       TODO Skill notes
       - Need to check Vergil stuff
       - Need to run through all skills again, see if there's any holes
       - To Test/Find
           - Rebellion - High Time, Float (Glide), Slash Roll (Pole Play with Rebellion)
           - A&R - Million Slash (Crazy Combo 3)
           - Nevan - Thunder Bolt and Vortex (Air raid abilities)
           - Beowulf - Tornado (Crazy RI)
           - E&I - Wild Stomp, Rain Storm
           - Shotgun - Point Blank, finisher for gun stinger
           - Artemis - Acid Rain
           - Spiral - Reflector
           - Kalina Ann - More Grapple testing, might be multistep
*/
bitflags! {
    pub struct Exp0: u32 {
        // Default                        0b1111_1111_1111_1111_0101_1110_0111_1111;
        const Unknown =                   0b0011_1110_1100_0010_0001_1000_0000_0001;
        // Combo 1 - Need all three in order to do combo 1
        const Rebellion_Combo_A_1 =       0b0000_0000_0000_0000_0000_0000_0000_0010;
        const Rebellion_Combo_A_2 =       0b0000_0000_0000_0000_0000_0000_0000_0100;
        const Rebellion_Combo_A_3 =       0b0000_0000_0000_0000_0000_0000_0000_1000;
        // Combo 2 - Need Combo A-1
        const Rebellion_Combo_B_1 =       0b0000_0000_0000_0000_0000_0000_0001_0000;
        const Yamato_Rapid_Slash_1 =      0b0000_0000_0000_0000_0000_0000_0001_0000;
        const Rebellion_Combo_B_2 =       0b0000_0000_0000_0000_0000_0000_0010_0000;
        const Yamato_Rapid_Slash_2 =      0b0000_0000_0000_0000_0000_0000_0010_0000;
        const Rebellion_Helm_Breaker =    0b0000_0000_0000_0000_0000_0000_0100_0000;
        // Store
        const Rebellion_Stinger_1 =       0b0000_0000_0000_0000_0000_0000_1000_0000;
        const Rebellion_Stinger_2 =       0b0000_0000_0000_0000_0000_0001_0000_0000;
        // Not confirmed
        const Yamato_Judgement_Cut_1 =    0b0000_0000_0000_0000_0000_0010_0000_0000;
        const Yamato_Judgement_Cut_2 =    0b0000_0000_0000_0000_0000_0100_0000_0000;
        const Rebellion_Drive =           0b0000_0000_0000_0000_0010_0000_0000_0000;
        // Needs Combo 2
        const Rebellion_Million_Stabs =   0b0000_0000_0000_0000_0100_0000_0000_0000;
        const Rebellion_Sword_Pierce =    0b0000_0000_0000_0000_1000_0000_0000_0000;
        // First half
        const Rebellion_Prop_Shredder_1 = 0b0000_0000_0000_0001_0000_0000_0000_0000;
        // Needs Sword Pierce
        const Unarmed_Kick =              0b0000_0000_0000_0100_0000_0000_0000_0000;
        // Aerial Wave
        const Rebellion_Aerial_Wave_1 =   0b0000_0000_0000_1000_0000_0000_0000_0000;
        const Rebellion_Aerial_Wave_2 =   0b0000_0000_0001_0000_0000_0000_0000_0000;
        const Rebellion_Aerial_Wave_3 =   0b0000_0000_0010_0000_0000_0000_0000_0000;
        // Needs prop shredder 1
        const Rebellion_Prop_Shredder_2 = 0b0000_0001_0000_0000_0000_0000_0000_0000;
        // Cerberus Combo 1
        const Cerberus_Combo_A_1 =        0b0100_0000_0000_0000_0000_0000_0000_0000;
        const Cerberus_Combo_A_2 =        0b1000_0000_0000_0000_0000_0000_0000_0000;

        const Cerberus_Combo_A_B =        0b1100_0000_0000_0000_0000_0000_0000_0000;
    }

    pub struct Exp1: u32 {
        // Default                        0b1010_0111_1111_1111_1010_1111_0101_1111;
        const Unknown =                   0b1000_0000_0000_0011_1000_0000_0000_0000;
        // Continued Cerberus Combo 1
        const Cerberus_Combo_A_3 =        0b0000_0000_0000_0000_0000_0000_0000_0001;
        const Cerberus_Combo_A_4 =        0b0000_0000_0000_0000_0000_0000_0000_0010;
        const Cerberus_Combo_A_5 =        0b0000_0000_0000_0000_0000_0000_0000_0100;
        const Force_Edge_Helm_Breaker =   0b0000_0000_0000_0000_0000_0000_0000_0100;
        // Cerberus Combo 2 - Needs A1 and A2
        const Cerberus_Combo_B_1 =        0b0000_0000_0000_0000_0000_0000_0000_1000;
        const Cerberus_Combo_B_2 =        0b0000_0000_0000_0000_0000_0000_0001_0000;
        const Cerberus_Windmill =         0b0000_0000_0000_0000_0000_0000_0010_0000;
        const Cerberus_Revolver_2 =       0b0000_0000_0000_0000_0000_0000_0100_0000;
        // Also Revolver Lv2
        const Force_Edge_Stinger_1 =      0b0000_0000_0000_0000_0000_0000_0100_0000;
        // Also Revolver
        const Force_Edge_Stinger_2 =      0b0000_0000_0000_0000_0000_0000_1000_0000;
        const Cerberus_Revolver =         0b0000_0000_0000_0000_0000_0000_1000_0000;
        const Force_Edge_Round_Trip =     0b0000_0000_0000_0000_0000_0001_0000_0000;
        const Cerberus_Swing =            0b0000_0000_0000_0000_0000_0001_0000_0000;
        // Combo 2 Crazy
        const Cerberus_Satellite =        0b0000_0000_0000_0000_0000_0010_0000_0000;
        // Style
        const Cerberus_Flicker =          0b0000_0000_0000_0000_0000_0100_0000_0000;
        const Cerberus_Air_Flicker =      0b0000_0000_0000_0000_0000_1000_0000_0000;
        const Cerberus_Crystal =          0b0000_0000_0000_0000_0001_0000_0000_0000;
        const Cerberus_Million_Carats =   0b0000_0000_0000_0000_0010_0000_0000_0000;
        const Cerberus_Ice_Age =          0b0000_0000_0000_0000_0100_0000_0000_0000;
        // Agni & Rudra Combo 1
        const AgniAndRudra_Combo_A_1 =    0b0000_0000_0000_0100_0000_0000_0000_0000;
        const AgniAndRudra_Combo_A_2 =    0b0000_0000_0000_1000_0000_0000_0000_0000;
        const AgniAndRudra_Combo_A_3 =    0b0000_0000_0001_0000_0000_0000_0000_0000;
        const AgniAndRudra_Combo_A_4 =    0b0000_0000_0010_0000_0000_0000_0000_0000;
        const AgniAndRudra_Combo_A_5 =    0b0000_0000_0100_0000_0000_0000_0000_0000;
        // Agni & Rudra Combo 2 - Requires A-1 to start
        const AgniAndRudra_Combo_B_1 =    0b0000_0000_1000_0000_0000_0000_0000_0000;
        const AgniAndRudra_Combo_B_2 =    0b0000_0001_0000_0000_0000_0000_0000_0000;
        // Agni & Rudra Combo 3 - Requires A-1 and B-1
        const AgniAndRudra_Combo_C =      0b0000_0010_0000_0000_0000_0000_0000_0000;
        const AgniAndRudra_JetStream_2 =  0b0000_0100_0000_0000_0000_0000_0000_0000;
        const AgniAndRudra_JetStream_3 =  0b0000_1000_0000_0000_0000_0000_0000_0000;
        const AgniAndRudra_Aerial_Cross = 0b0010_0000_0000_0000_0000_0000_0000_0000;
        const AgniAndRudra_Whirlwind =    0b0100_0000_0000_0000_0000_0000_0000_0000;
    }

    pub struct Exp2: u32 {
        // Default                        0b1010_1111_0001_1111_1111_1111_1111_0011;
        const Unknown =                   0b1000_0000_0000_1100_1111_1111_1000_0001;
        const AgniAndRudra_Cross_Swords = 0b0000_0000_0000_0000_0000_0000_0000_0010;
        // Both Twister+Tempest?
        const AgniAndRudra_Crawler =      0b0000_0000_0000_0000_0000_0000_0000_0100;
        const AgniAndRudra_Tempest =      0b0000_0000_0000_0000_0000_0000_0000_1000;
        // Agni and Rudra - Sky Dance
        // First slashes
        const AgniAndRudra_Sky_Dance_1 =  0b0000_0000_0000_0000_0000_0000_0001_0000;
        // Second slashes, seems to be fine on its own
        const AgniAndRudra_Sky_Dance_2 =  0b0000_0000_0000_0000_0000_0000_0010_0000;
        // Spin down to the ground, independent
        const AgniAndRudra_Sky_Dance_3 =  0b0000_0000_0000_0000_0000_0000_0100_0000;
        const Nevan_Tune_Up =             0b0000_0000_0000_0001_0000_0000_0000_0000;
        // Needs Tune Up, Also includes Jam Session
        const Neva_Combo_ABC =            0b0000_0000_0000_0010_0000_0000_0000_0000;
        const Nevan_Bat_Rift =            0b0000_0000_0001_0000_0000_0000_0000_0000;
        const Nevan_Bat_Rift_2 =          0b0000_0000_0010_0000_0000_0000_0000_0000;
        const Nevan_Reverb_Shock_1 =      0b0000_0000_0100_0000_0000_0000_0000_0000;
        const Nevan_Reverb_Shock_2 =      0b0000_0000_1000_0000_0000_0000_0000_0000;
        const Nevan_Air_Play =            0b0000_0001_0000_0000_0000_0000_0000_0000;
        const Nevan_Slash =               0b0000_0010_0000_0000_0000_0000_0000_0000;
        const Nevan_Air_Slash =           0b0000_0100_0000_0000_0000_0000_0000_0000;
        // Style
        const Nevan_Feedback =            0b0001_0000_0000_0000_0000_0000_0000_0000;
        const Nevan_Crazy_Roll =          0b0010_0000_0000_0000_0000_0000_0000_0000;
        const Nevan_Distortion =          0b0100_0000_0000_0000_0000_0000_0000_0000;
    }

    pub struct Exp3: u32 {
        // Default                        0b1100_1011_1001_1111_1111_1111_1111_1001;
        const Unknown =                   0b0100_0001_1000_0000_0001_1111_1111_1001;
        const Nevan_Volume_Up =           0b0000_0000_0000_0000_0000_0000_0000_0010;
        const Nevan_Air_Raid =            0b0000_0000_0000_0000_0000_0000_0000_0100;
        // Beowulf Combo 1
        const Beowulf_Combo_A_1 =         0b0000_0000_0000_0000_0100_0000_0000_0000;
        const Beowulf_Combo_A_2 =         0b0000_0000_0000_0000_1000_0000_0000_0000;
        const Beowulf_Combo_A_3 =         0b0000_0000_0000_0001_0000_0000_0000_0000;
        // Beowulf Combo 2 - Needs A-1 and A-2
        const Beowulf_Combo_B_1 =         0b0000_0000_0000_0010_0000_0000_0000_0000;
        const Beowulf_Combo_B_2 =         0b0000_0000_0000_0100_0000_0000_0000_0000;
        // Crazy Combo for Beowulf Combo 2, doesn't need B-2
        const Beowulf_Combo_Hyper_Fist =  0b0000_0000_0000_1000_0000_0000_0000_0000;
        const Beowulf_Killer_Bee =        0b0000_0000_0001_0000_0000_0000_0000_0000;
        const Beowulf_Beast_Uppercut =    0b0000_0000_0010_0000_0000_0000_0000_0000;
        const Beowulf_Rising_Dragon =     0b0000_0000_0100_0000_0000_0000_0000_0000;
        // Might be backwards? Both appear to be straight
        const Beowulf_Straight_2 =        0b0000_0010_0000_0000_0000_0000_0000_0000;
        const Beowulf_Straight_Q =        0b0000_0100_0000_0000_0000_0000_0000_0000;
        // Style
        const Beowulf_Zodiac =            0b0000_1000_0000_0000_0000_0000_0000_0000;
        const Beowulf_Ground_Volcano =    0b0001_0000_0000_0000_0000_0000_0000_0000;
        const Beowulf_Air_Volcano =       0b0010_0000_0000_0000_0000_0000_0000_0000;
        const Beowulf_Hammer =            0b1000_0000_0000_0000_0000_0000_0000_0000;
    }

    pub struct Exp4: u32 {
        // Default                        0b1111_1011_1111_1011_1111_1111_1111_1110;
        const Unknown =                   0b1111_1010_0011_1000_1000_1111_1010_0010;
        const Beowulf_Real_Impact =       0b0000_0000_0000_0000_0000_0000_0000_0001;
        const Ebony_And_Ivory_Shoot =     0b0000_0000_0000_0000_0000_0000_0000_0100;
        const EbonyIvory_Charge_Shot =    0b0000_0000_0000_0000_0000_0000_0000_1000;
        const EbonyIvory_Air_Shoot =      0b0000_0000_0000_0000_0000_0000_0001_0000;
        const Gunslinger_Twosome_Time =   0b0000_0000_0000_0000_0000_0000_0100_0000;
        const Shotgun_Shoot =             0b0000_0000_0000_0000_0001_0000_0000_0000;
        const Shotgun_Charge_Shot =       0b0000_0000_0000_0000_0010_0000_0000_0000;
        const Shotgun_Air_Shoot =         0b0000_0000_0000_0000_0100_0000_0000_0000;
        const Shotgun_Fireworks =         0b0000_0000_0000_0001_0000_0000_0000_0000;
        const Shotgun_Fireworks_Air =     0b0000_0000_0000_0010_0000_0000_0000_0000;
        const Shotgun_Stinger =           0b0000_0000_0000_0100_0000_0000_0000_0000;
        const Artemis_Shoot =             0b0000_0000_0100_0000_0000_0000_0000_0000;
        const Artemis_Air_Shoot =         0b0000_0000_1000_0000_0000_0000_0000_0000;
        const Artemis_Multilock =         0b0000_0001_0000_0000_0000_0000_0000_0000;
        const Artemis_Sphere =            0b0000_0100_0000_0000_0000_0000_0000_0000;
    }

    pub struct Exp5: u32 {
         // Default                       0b1111_1111_1111_1111_1110_1111_1111_1101;
        const Unknown =                   0b1001_1111_0000_1111_1110_0011_1111_1000;
        const Spiral_Shoot =              0b0000_0000_0000_0000_0000_0000_0000_0001;
        const Spiral_Sniper =             0b0000_0000_0000_0000_0000_0000_0000_0010;
        const Spiral_Trick_Shot =         0b0000_0000_0000_0000_0000_0000_0000_0100;
        const Kalina_Ann_Shoot =          0b0000_0000_0000_0000_0000_0100_0000_0000;
        const Kalina_Ann_Hysteria =       0b0000_0000_0000_0000_0000_1000_0000_0000;
        const Kalina_Ann_Grapple =        0b0000_0000_0000_0000_0001_0000_0000_0000;
        // Amount of dashes seems to depend on style level, not additional flags
        const Trickster_Dash =            0b0000_0000_0001_0000_0000_0000_0000_0000;
        const Trickster_Sky_Star =        0b0000_0000_0010_0000_0000_0000_0000_0000;
        const Trickster_Air_Trick =       0b0000_0000_0100_0000_0000_0000_0000_0000;
        const Trickster_Wall_Hike =       0b0000_0000_1000_0000_0000_0000_0000_0000;
        const Royalguard_Release =        0b0100_0000_0000_0000_0000_0000_0000_0000;
    }

    pub struct Exp6: u32 {
        // Default                        0b1111_1111_1110_0011_1111_1110_1111_1111;
        const Unknown =                   0b1111_1111_1110_0000_0000_0000_1111_1111;
        const Royalguard_Release_Air =    0b0000_0000_0000_0000_0000_0000_0000_0100;
        // The wall of dance macabre
        const Dance_Macabre_1 =           0b0000_0000_0000_0000_0000_0001_0000_0000;
        const Dance_Macabre_2 =           0b0000_0000_0000_0000_0000_0010_0000_0000;
        const Dance_Macabre_3 =           0b0000_0000_0000_0000_0000_0100_0000_0000;
        const Dance_Macabre_4 =           0b0000_0000_0000_0000_0000_1000_0000_0000;
        const Dance_Macabre_5 =           0b0000_0000_0000_0000_0001_0000_0000_0000;
        const Dance_Macabre_6 =           0b0000_0000_0000_0000_0010_0000_0000_0000;
        const Dance_Macabre_7 =           0b0000_0000_0000_0000_0100_0000_0000_0000;
        const Dance_Macabre_8 =           0b0000_0000_0000_0000_1000_0000_0000_0000;
        const Dance_Macabre_9 =           0b0000_0000_0000_0001_0000_0000_0000_0000;
        //const Dance_Macabre_All =       0b0000_0000_0000_0001_1111_1111_0000_0000;
        const Poll_Play =                 0b0000_0000_0000_0010_0000_0000_0000_0000;
        const Rebellion_Air_Hike =        0b0000_0000_0000_0100_0000_0000_0000_0000;
        const AgniAndRudra_Air_Hike =     0b0000_0000_0000_1000_0000_0000_0000_0000;
        const Beowulf_Air_Hike =          0b0000_0000_0001_0000_0000_0000_0000_0000;
    }

    pub struct Exp7: u32 { // ??
         // Default                       0b1111_1111_1111_1111_1111_1111_1111_1111;
         const Unknown =                  0b0000_0000_0000_0000_0000_0000_0000_0000;
    }
}

pub(crate) fn reset_expertise() {
    SessionData::with_mut(|s| {
        s.expertise = DEFAULT_SKILLS;
    })
    .expect("Unable to reset expertise");
    let _ = CharacterData::with_mut(|c| {
        c.expertise = DEFAULT_SKILLS;
    });
}

fn give_skill(skill_id: &usize) {
    // This works, might not update files? need to double-check
    let data = SKILLS_MAP.get(ID_SKILL_MAP.get(skill_id).unwrap()).unwrap();
    SessionData::with_mut(|s| {
        s.expertise[data.index].bitor_assign(data.flag);
    })
    .expect("Unable to give skill");

    let _ = CharacterData::with_mut(|c| {
        c.expertise[data.index].bitor_assign(data.flag);
    });
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
        0x40 => {
            data.add_stinger_level();
        }
        0x46 => {
            data.add_jet_stream_level();
        }
        0x4A => {
            data.add_reverb_level();
        }
        0x50 => {
            data.add_beowulf_level();
        }
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
        _ => id,
    };
    data.add_skill(skill_id);
}
