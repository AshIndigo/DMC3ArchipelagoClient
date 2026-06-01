use crate::config;
use minhook::MinHook;
use pelite::pattern;
use pelite::pe64::Pe;
use pelite::pe64::PeView;
use randomizer_utilities::get_base_address;
use randomizer_utilities::BasicNothingFunc;
use std::sync::{LazyLock, OnceLock};

pub static CRIMSON_ADDRESS: LazyLock<usize> = LazyLock::new(|| get_base_address("Crimson.dll"));

static ORIGINAL_FIX_WEAPON_UNLOCKS_DANTE: OnceLock<BasicNothingFunc> = OnceLock::new();
static FIX_WEAPON_UNLOCKS_DANTE_ADDR: LazyLock<Option<usize>> =
    LazyLock::new(search_fix_weapon_unlocks_address);

pub fn setup_crimson_hook() {
    if !config::CONFIG.mods.disable_crimson_hooks {
        log::info!("Starting up Crimson hook");
        log::info!("Crimson base ADDR: {:X}", *CRIMSON_ADDRESS);
        match *FIX_WEAPON_UNLOCKS_DANTE_ADDR {
            None => {
                log::error!("Unable to find FixWeaponUnlocksDante address!")
            }
            Some(addr) => {
                log::debug!("Disabling FixWeaponUnlocksDante function");
                unsafe {
                    init_weapon_unlock_hook(addr);
                    MinHook::enable_hook((*CRIMSON_ADDRESS + addr) as _) // FixWeaponUnlocksDante
                        .expect("Failed to enable FixWeaponUnlocksDante hook");
                }
            }
        }
        log::info!("Crimson hook initialized");
    } else {
        log::info!("Crimson is detected but hooks will not be enabled")
    }
}

fn search_fix_weapon_unlocks_address() -> Option<usize> {
    let view = unsafe { PeView::module(*CRIMSON_ADDRESS as *const u8) };
    let scanner = view.scanner();
    let mut save = [0; 1];
    // Starting bytes for FixWeaponsUnlockDante
    // Hopefully this is stable?
    let pat = pattern!(
        "40 ?
         41 ?
         48 83 EC 78
         48 8b ? ? ? ? ?"
    );
    if scanner.finds_code(pat, &mut save) {
        log::debug!("FixWeaponUnlocksDante Offset: {:#X}", save[0]);
        return Some(save[0].try_into().unwrap());
    }
    None
}

fn init_weapon_unlock_hook(addr: usize) {
    ORIGINAL_FIX_WEAPON_UNLOCKS_DANTE.get_or_init(|| unsafe {
        std::mem::transmute::<_, BasicNothingFunc>(
            MinHook::create_hook((*CRIMSON_ADDRESS + addr) as _, dont_fix_weapons as _)
                .expect("Failed to create hook"),
        )
    });
}

fn dont_fix_weapons() {}
