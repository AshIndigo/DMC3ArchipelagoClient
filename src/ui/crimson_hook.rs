use std::sync::LazyLock;
use minhook::MinHook;
use crate::{config, utilities};

pub static CRIMSON_ADDRESS: LazyLock<usize> = LazyLock::new(|| utilities::get_base_address("Crimson.dll"));

pub fn setup_crimson_hook() {
    if !(*config::CONFIG).mods.disable_crimson_hooks {
        log::info!("Starting up Crimson hook");
        log::info!("Crimson base ADDR: {:X}", *CRIMSON_ADDRESS);
        unsafe {
            // MinHook::enable_hook((*CRIMSON_ADDRESS + 0x05) as _)
            //     .expect("Failed to enable timestep hook");
        }
        log::info!("Crimson hook initialized");
    } else {
        log::info!("Crimson is detected but hooks will not be enabled")
    }
}

// Since Crimson is in active development, I can't just hard code addresses. I could probably get away with exported functions