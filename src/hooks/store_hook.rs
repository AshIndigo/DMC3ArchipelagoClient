use crate::DMC3_ADDRESS;
use crate::constants::{EMPTY_COORDINATES, GUN_NAMES};
use crate::hooks::check_handler;
use crate::hooks::check_handler::{Location, LocationType};
use crate::mapping::MAPPING;
use crate::ui::overlay::CANT_PURCHASE;
use crate::{AP_CORE, create_hook, tracker};
use minhook::{MH_STATUS, MinHook};
use randomizer_utilities::read_data_from_address;
use std::ptr::write;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, OnceLock, RwLock};

pub fn create_hooks() -> Result<(), MH_STATUS> {
    unsafe {
        create_hook!(
            FRAME_SKILL_SHOP_ADDR,
            skill_shop_frame,
            ORIGINAL_FRAME_SKILL_SHOP,
            "Deny purchases of skills"
        );
        create_hook!(
            SKILL_SHOP_CONSTRUCTOR,
            skill_shop_constructor,
            ORIGINAL_SKILL_SHOP_CONSTRUCTOR,
            "Skill purchase constructor"
        );
        create_hook!(
            SKILL_SHOP_DECONSTRUCTOR,
            skill_shop_deconstructor,
            ORIGINAL_SKILL_SHOP_DECONSTRUCTOR,
            "Skill purchase deconstructor"
        );
        create_hook!(
            GUN_SHOP_CONSTRUCTOR_ADDR,
            gun_upgrade_constructor,
            ORIGINAL_GUN_SHOP_CONSTRUCTOR_ADDR,
            "Gun Upgrade Screen Constructor (D)"
        );
        create_hook!(
            GUN_SHOP_EXIT_ADDR,
            exit_gun_upgrade_screen,
            ORIGINAL_GUN_SHOP_EXIT,
            "Gun Upgrade Screen Destructor (D)"
        );
        create_hook!(
            SOMETHING_GUN_ADDR,
            something_gun,
            ORIGINAL_SOMETHING_GUN,
            "Gun Screen related (D)"
        );
        create_hook!(
            GUN_PURCHASE_CONFIRMATION_ADDR,
            gun_upgrade_confirm,
            ORIGINAL_GUN_PURCHASE_CONFIRMATION,
            "Tracker updating and gun level checks"
        );
        create_hook!(
            ADD_SHOTGUN_OR_CERBERUS_ADDR,
            deny_cerberus_or_shotgun,
            ORIGINAL_ADD_SHOTGUN_OR_CERBERUS,
            "Don't add the Shotgun/Cerberus to second slot"
        );
        create_hook!(
            PURCHASE_HP_ADDR,
            deny_blue_or_purple_orb,
            ORIGINAL_PURCHASE_HP,
            "N/A"
        );
        create_hook!(
            PURCHASE_DT_ADDR,
            deny_blue_or_purple_orb,
            ORIGINAL_PURCHASE_DT,
            "N/A"
        );
    }
    Ok(())
}

pub(crate) static HOOK_ADDRESSES: LazyLock<Vec<usize>> = LazyLock::new(|| {
    const ADDRESSES: [usize; 10] = [
        SKILL_SHOP_CONSTRUCTOR,
        SKILL_SHOP_DECONSTRUCTOR,
        FRAME_SKILL_SHOP_ADDR,
        GUN_SHOP_CONSTRUCTOR_ADDR,
        GUN_SHOP_EXIT_ADDR,
        SOMETHING_GUN_ADDR,
        GUN_PURCHASE_CONFIRMATION_ADDR,
        ADD_SHOTGUN_OR_CERBERUS_ADDR,
        PURCHASE_DT_ADDR,
        PURCHASE_HP_ADDR,
    ];
    ADDRESSES.to_vec()
});

pub const SKILL_SHOP_CONSTRUCTOR: usize = 0x287dd0;
pub static ORIGINAL_SKILL_SHOP_CONSTRUCTOR: OnceLock<unsafe extern "C" fn(custom_skill: usize)> =
    OnceLock::new();

pub fn skill_shop_constructor(custom_skill: usize) {
    if let Some(orig) = ORIGINAL_SKILL_SHOP_CONSTRUCTOR.get() {
        unsafe {
            orig(custom_skill);
        }
    }
    if let Some(mapping) = MAPPING.read().unwrap().as_ref()
        && mapping.randomize_skills
        && !mapping.shop_skill_checks
    {
        CANT_PURCHASE.store(true, Ordering::SeqCst);
    }
}

pub const SKILL_SHOP_DECONSTRUCTOR: usize = 0x287f20;
pub static ORIGINAL_SKILL_SHOP_DECONSTRUCTOR: OnceLock<unsafe extern "C" fn(custom_skill: usize)> =
    OnceLock::new();

pub fn skill_shop_deconstructor(custom_skill: usize) {
    if let Some(orig) = ORIGINAL_SKILL_SHOP_DECONSTRUCTOR.get() {
        unsafe {
            orig(custom_skill);
        }
    }
    CANT_PURCHASE.store(false, Ordering::SeqCst);
}

pub const FRAME_SKILL_SHOP_ADDR: usize = 0x288280;
pub static ORIGINAL_FRAME_SKILL_SHOP: OnceLock<unsafe extern "C" fn(custom_skill: usize)> =
    OnceLock::new();

pub fn skill_shop_frame(custom_skill: usize) {
    if let Some(mapping) = MAPPING.read().unwrap().as_ref()
        && mapping.randomize_skills
    {
        if read_data_from_address::<u8>(custom_skill + 0x08) == 0x05 {
            //unsafe { replace_single_byte(custom_skill + 0x08, 0x01) }
        }
    }

    if let Some(orig) = ORIGINAL_FRAME_SKILL_SHOP.get() {
        unsafe {
            orig(custom_skill);
        }
    }
}

pub static BACKUP_LEVELS: RwLock<[u8; 5]> = RwLock::new([0; 5]);
pub const SOMETHING_GUN_ADDR: usize = 0x282fa0;
pub static ORIGINAL_SOMETHING_GUN: OnceLock<unsafe extern "C" fn(custom_gun: usize)> =
    OnceLock::new();

// TODO Better name for this
pub fn something_gun(custom_gun: usize) {
    if let Some(orig) = ORIGINAL_SOMETHING_GUN.get() {
        unsafe {
            orig(custom_gun);
        }
    }
    match BACKUP_LEVELS.try_write() {
        Ok(mut levels) => {
            *levels = read_data_from_address(custom_gun + 0x3d10);
        }
        Err(err) => {
            log::error!("Failed to lock gun backup levels: {}", err);
        }
    }
    if let Some(mapping) = MAPPING.read().unwrap().as_ref() {
        // Shop gun check purchases need to be at the current check level, not gun level
        // Gun levels as items means they aren't purchased
        if mapping.shop_gun_checks && mapping.randomize_gun_levels {
            // What gun level checks have we done
            // The shop will display the last purchased gun level check
            unsafe {
                write(
                    (custom_gun + 0x3D10) as *mut [u8; 5],
                    get_gun_level_checks(),
                );
            }
        }
    }
}

pub const GUN_SHOP_CONSTRUCTOR_ADDR: usize = 0x2825d0;
pub static ORIGINAL_GUN_SHOP_CONSTRUCTOR_ADDR: OnceLock<unsafe extern "C" fn(custom_gun: usize)> =
    OnceLock::new();

pub fn gun_upgrade_constructor(custom_gun: usize) {
    // Run original code
    if let Some(orig) = ORIGINAL_GUN_SHOP_CONSTRUCTOR_ADDR.get() {
        unsafe {
            orig(custom_gun);
        }
    }
    if let Some(mapping) = MAPPING.read().unwrap().as_ref()
        && mapping.randomize_gun_levels
        && !mapping.shop_gun_checks
    {
        CANT_PURCHASE.store(true, Ordering::SeqCst);
    }
}

pub const GUN_SHOP_EXIT_ADDR: usize = 0x283080;
//0x2826a0;
pub static ORIGINAL_GUN_SHOP_EXIT: OnceLock<unsafe extern "C" fn(custom_gun: usize)> =
    OnceLock::new();

pub fn exit_gun_upgrade_screen(custom_gun: usize) -> bool {
    CANT_PURCHASE.store(false, Ordering::SeqCst);

    if let Some(mapping) = MAPPING.read().unwrap().as_ref()
        && mapping.shop_gun_checks
    {
        unsafe {
            write(
                (custom_gun + 0x3D10) as *mut [u8; 5],
                *BACKUP_LEVELS.read().unwrap(),
            );
        }
    }

    // Run original code
    if let Some(orig) = ORIGINAL_GUN_SHOP_EXIT.get() {
        unsafe {
            orig(custom_gun);
        }
    }
    false
}
pub const GUN_PURCHASE_CONFIRMATION_ADDR: usize = 0x2833b0;
pub static ORIGINAL_GUN_PURCHASE_CONFIRMATION: OnceLock<unsafe extern "C" fn(custom_gun: usize)> =
    OnceLock::new();

// When actually selecting a gun
pub fn gun_upgrade_confirm(custom_gun: usize) {
    // What gun level checks have we done
    let backup_gun_levels = read_data_from_address::<[u8; 5]>(custom_gun + 0x3D10);
    if let Some(mapping) = MAPPING.read().unwrap().as_ref() {
        // If randomize gun levels and no checks for them, deny purchasing
        if mapping.randomize_gun_levels && !mapping.shop_gun_checks {
            return;
        }
        // Run original code
        if let Some(orig) = ORIGINAL_GUN_PURCHASE_CONFIRMATION.get() {
            unsafe {
                orig(custom_gun);
            }
        }
        // TODO Overlay to show actual gun levels as well
        let gun_levels_new = read_data_from_address::<[u8; 5]>(custom_gun + 0x3D10);
        for gun_idx in 0..5 {
            if gun_levels_new[gun_idx] > backup_gun_levels[gun_idx] {
                log::debug!(
                    "Attempting to purchase gun upgrade: {} - LV{}",
                    gun_idx,
                    gun_levels_new[gun_idx]
                );
                // Tracker updates
                // Don't send updates if gun levels are items
                if !mapping.randomize_gun_levels
                    && let Ok(mut core) = AP_CORE.get().unwrap().as_ref().lock()
                    && let Some(client) = core.connection.client_mut()
                    && let Err(e) =
                        tracker::GunLevels::update(gun_idx, gun_levels_new[gun_idx], client)
                {
                    log::error!("Failed to update GunLevels: {}", e);
                }

                // Send out check for purchasing gun upgrade
                if mapping.shop_gun_checks {
                    check_handler::send_off_location_coords(
                        Location {
                            location_type: LocationType::PurchaseItem,
                            item_id: match gun_idx {
                                0 => 0x1C,
                                1 => 0x1D,
                                2 => 0x1E,
                                3 => 0x1F,
                                4 => 0x21,
                                _ => unreachable!(),
                            },
                            mission: 1 + gun_levels_new[gun_idx] as u32,
                            room: 0,
                            coordinates: EMPTY_COORDINATES,
                            to_display: true,
                        },
                        u32::MAX,
                    );
                }
            }
        }
    }
}

/// Returns the number of purchased gun upgrades checks per gun.
fn get_gun_level_checks() -> [u8; 5] {
    let mut res = [0u8; 5];

    if let Some(client) = AP_CORE.get().unwrap().lock().unwrap().connection.client() {
        let checked = client.checked_locations().collect::<Vec<_>>();

        for (i, gun) in GUN_NAMES.iter().enumerate() {
            let level_2 = format!("Purchase {} Level 2", gun);
            let level_3 = format!("Purchase {} Level 3", gun);

            let has_level_2 = checked.iter().any(|loc| loc.name() == level_2);
            let has_level_3 = checked.iter().any(|loc| loc.name() == level_3);

            res[i] = match (has_level_2, has_level_3) {
                (true, true) => 2,
                (true, false) => 1,
                _ => 0,
            };
        }
    }

    res
}

pub const ADD_SHOTGUN_OR_CERBERUS_ADDR: usize = 0x1fcfa0;
pub static ORIGINAL_ADD_SHOTGUN_OR_CERBERUS: OnceLock<
    unsafe extern "C" fn(custom_gun: usize, id: u8) -> bool,
> = OnceLock::new();

// Disabling vanilla behavior of inserting the shotgun/cerberus into the second weapon slot
pub fn deny_cerberus_or_shotgun(_param_1: usize, _id: u8) -> bool {
    false
}

// Making it so purchasing a blue/purple orb in the store does not give the relevant stat boost
pub static ORIGINAL_PURCHASE_HP: OnceLock<PurchaseStatOrb> = OnceLock::new();
pub static ORIGINAL_PURCHASE_DT: OnceLock<PurchaseStatOrb> = OnceLock::new();

type PurchaseStatOrb = unsafe fn(ptr: usize, hp: f32) -> f32;

pub const PURCHASE_HP_ADDR: usize = 0x86e90;
pub const PURCHASE_DT_ADDR: usize = 0x86e30;

pub fn deny_blue_or_purple_orb(_param_1: usize, _amount: f32) -> f32 {
    1000.0
}
