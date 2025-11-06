use core::sync::atomic::Ordering;
use crate::constants::BasicNothingFunc;
use crate::{config, utilities};
use minhook::MinHook;
use std::sync::{LazyLock, OnceLock};
use std::sync::atomic::AtomicBool;

pub static CRIMSON_ADDRESS: LazyLock<usize> =
    LazyLock::new(|| utilities::get_base_address("Crimson.dll"));

static ORIGINAL_FIX_WEAPON_UNLOCKS_DANTE: OnceLock<BasicNothingFunc> = OnceLock::new();
static FIX_WEAPON_UNLOCKS_DANTE_ADDR: usize = 0x1EBD00;

pub static CRIMSON_HASH_ISSUE: AtomicBool = AtomicBool::new(false);

// Since Crimson is in active development, I shouldn't just hard code addresses. I could probably get away with exported functions

pub fn setup_crimson_hook() {
    if !config::CONFIG.mods.disable_crimson_hooks || !CRIMSON_HASH_ISSUE.load(Ordering::SeqCst) {
        log::info!("Starting up Crimson 0.4 hook");
        log::info!("Crimson base ADDR: {:X}", *CRIMSON_ADDRESS);
        unsafe {
            log::debug!("Disabling FixWeaponUnlocksDante function");
            init_weapon_unlock_hook();
            MinHook::enable_hook((*CRIMSON_ADDRESS + FIX_WEAPON_UNLOCKS_DANTE_ADDR) as _) // FixWeaponUnlocksDante
                .expect("Failed to enable FixWeaponUnlocksDante hook");
        }
        log::info!("Crimson hook initialized");
    } else {
        log::info!("Crimson is detected but hooks will not be enabled")
    }
}

fn init_weapon_unlock_hook() {
    ORIGINAL_FIX_WEAPON_UNLOCKS_DANTE.get_or_init(|| unsafe {
        std::mem::transmute::<_, BasicNothingFunc>(
            MinHook::create_hook(
                (*CRIMSON_ADDRESS + FIX_WEAPON_UNLOCKS_DANTE_ADDR) as _,
                dont_fix_weapons as _,
            )
            .expect("Failed to create hook"),
        )
    });
}

fn dont_fix_weapons() {
}
