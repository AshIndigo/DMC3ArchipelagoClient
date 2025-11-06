use std::collections::HashSet;
use crate::constants::{BasicNothingFunc, ItemCategory};
use crate::compat::imgui_bindings::*;
use crate::ui::ui::get_status_text;
use crate::utilities::read_data_from_address;
use crate::{bank, check_handler, config, constants, game_manager, utilities};
use imgui_sys::{ImGuiCond, ImGuiCond_Appearing, ImGuiWindowFlags, ImVec2};
use minhook::MinHook;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::thread;
use crate::ui::text_handler;

pub static MARY_ADDRESS: LazyLock<usize> =
    LazyLock::new(|| utilities::get_base_address("Mary.dll"));
static SETUP: AtomicBool = AtomicBool::new(false);

pub const USE_2022_DDMK: bool = true;

const MAIN_FUNC_ADDR: usize = if USE_2022_DDMK { 0xC17B0 } else { 0xcb3c0 }; //0xC65E0; // 0xC17B0 (For 2022 ddmk)
const TIMESTEP_FUNC_ADDR: usize = if USE_2022_DDMK { 0x1DC50 } else { 0x01de20 }; //0x1DE20; // 0x1DC50 (For 2022 ddmk)
const DDMK_UI_ENABLED: usize = if USE_2022_DDMK { 0x1258da } else { 0x13374a }; //0x12c73a; (Probably old indigo ver.)

unsafe extern "C" fn hooked_timestep() {
    unsafe {
        if !SETUP.load(Ordering::SeqCst) {
            MinHook::enable_hook((*MARY_ADDRESS + MAIN_FUNC_ADDR) as _)
                .expect("Failed to enable hook");
            SETUP.store(true, Ordering::SeqCst);
        }
        match get_orig_timestep_func() {
            None => {
                panic!("ORIG_TIMESTEP_FUNC not initialized in hooked render");
            }
            Some(timestep_func) => {
                timestep_func();
            }
        }
    }
}

unsafe extern "C" fn hooked_render() {
    unsafe {
        if !SETUP.load(Ordering::SeqCst) {
            return;
        }

        if !read_data_from_address::<bool>(DDMK_UI_ENABLED + *MARY_ADDRESS) {
            return;
        }
        archipelago_window(); // For the archipelago window
        tracking_window();
        bank_window();
        match get_orig_render_func() {
            None => {}
            Some(fnc) => {
                fnc();
            }
        }
    }
}

unsafe fn tracking_window() {
    unsafe {
        let flag = &mut true;
        get_imgui_next_pos()(
            &ImVec2 { x: 800.0, y: 320.0 }, // 300
            ImGuiCond_Appearing as ImGuiCond,
            &ImVec2 { x: 0.0, y: 0.0 },
        );
        get_imgui_begin()(
            c"Tracker".as_ptr() as *const c_char,
            flag as *mut bool,
            imgui_sys::ImGuiWindowFlags_AlwaysAutoResize as ImGuiWindowFlags,
        );

        match game_manager::ARCHIPELAGO_DATA.read() {
            Ok(data) => {
                for chunk in constants::get_items_by_category(ItemCategory::Key).chunks(3) {
                    let row_text = chunk
                        .iter()
                        .map(|&item| checkbox_text(item, &data.items))
                        .collect::<Vec<String>>()
                        .join("  ");
                    text(format!("{}\0", row_text));
                }
                text(format!(
                    "Blue Orbs: {}\0",
                    data.blue_orbs
                ));
                text(format!(
                    "Purple Orbs: {}\0",
                    data.purple_orbs
                ));
            }
            Err(err) => {
                log::error!("Failed to read ArchipelagoData: {:?}", err);
            }
        }

        get_imgui_end()();
    }
}

unsafe fn bank_window() {
    unsafe {
        let flag = &mut true;
        get_imgui_next_pos()(
            &ImVec2 { x: 800.0, y: 500.0 },
            ImGuiCond_Appearing as ImGuiCond,
            &ImVec2 { x: 0.0, y: 0.0 },
        );
        get_imgui_begin()(
            c"Bank".as_ptr() as *const c_char,
            flag as *mut bool,
            imgui_sys::ImGuiWindowFlags_AlwaysAutoResize as ImGuiWindowFlags,
        );
        let consumables = constants::get_items_by_category(ItemCategory::Consumable);
        for n in 0..constants::get_items_by_category(ItemCategory::Consumable).len() {
            // Special case for red orbs...
            let item = consumables.get(n).unwrap();
            text(format!(
                "{}: {}\0",
                item,
                bank::get_bank().read().unwrap().get(item).unwrap()
            ));
        }
        get_imgui_end()();
    }
}

fn checkbox_text(item: &str, list: &HashSet<&str>) -> String {
    format!("{} [{}]", item, if list.contains(&item) { "X" } else { " " })
}

pub unsafe fn archipelago_window() {
    unsafe {
        let flag = &mut true;
        get_imgui_next_pos()(
            &ImVec2 { x: 800.0, y: 100.0 },
            ImGuiCond_Appearing as ImGuiCond,
            &ImVec2 { x: 0.0, y: 0.0 },
        );
        get_imgui_begin()(
            c"Archipelago".as_ptr() as *const c_char,
            flag as *mut bool,
            imgui_sys::ImGuiWindowFlags_AlwaysAutoResize as ImGuiWindowFlags,
        );
        text(format!("Status: {}\0", get_status_text()));
        const DEBUG: bool = false;
        if DEBUG {
            if get_imgui_button()(
                c"Clear dummy flags".as_ptr() as *const c_char,
                &ImVec2 { x: 0.0, y: 0.0 },
            ) {
                thread::spawn(move || {
                    check_handler::clear_high_roller();
                });
            }
            if get_imgui_button()(
                c"Kill Dante".as_ptr() as *const c_char,
                &ImVec2 { x: 0.0, y: 0.0 },
            ) {
                thread::spawn(move || {
                    game_manager::kill_dante();
                });
            }
            // if get_imgui_button()(
            //     "Modify Health\0".as_ptr() as *const c_char,
            //     &ImVec2 { x: 0.0, y: 0.0 },
            // ) {
            //     thread::spawn(move || {
            //         utilities::give_hp(constants::ONE_ORB);
            //     });
            // }
            if get_imgui_button()(
                c"Edit Message Index".as_ptr() as *const c_char,
                &ImVec2 { x: 0.0, y: 0.0 },
            ) {
                thread::spawn(move || {
                    text_handler::display_message_via_index("Testing test");
                });
            }
        }
        get_imgui_end()();
    }
}

pub fn setup_ddmk_hook() {
    if !config::CONFIG.mods.disable_ddmk_hooks {
        log::info!("Starting up DDMK hook");
        log::info!("Mary base ADDR: {:X}", *MARY_ADDRESS);
        init_render_func();
        init_timestep_func();
        unsafe {
            MinHook::enable_hook((*MARY_ADDRESS + TIMESTEP_FUNC_ADDR) as _)
                .expect("Failed to enable timestep hook");
        }
        log::info!("DDMK hook initialized");
    } else {
        log::info!("DDMK is detected but hooks will not be enabled")
    }
}

static ORIG_RENDER_FUNC: OnceLock<Option<BasicNothingFunc>> = OnceLock::new();

fn init_render_func() {
    ORIG_RENDER_FUNC.get_or_init(|| {
        Some(unsafe {
            std::mem::transmute::<_, BasicNothingFunc>(
                MinHook::create_hook((*MARY_ADDRESS + MAIN_FUNC_ADDR) as _, hooked_render as _)
                    .expect("Failed to create hook"),
            )
        })
    });
}

fn get_orig_render_func() -> Option<BasicNothingFunc> {
    *ORIG_RENDER_FUNC.get().unwrap_or(&None)
}

static ORIG_TIMESTEP_FUNC: OnceLock<Option<BasicNothingFunc>> = OnceLock::new();

fn init_timestep_func() {
    ORIG_TIMESTEP_FUNC.get_or_init(|| {
        Some(unsafe {
            std::mem::transmute::<_, BasicNothingFunc>(
                MinHook::create_hook(
                    (*MARY_ADDRESS + TIMESTEP_FUNC_ADDR) as _,
                    hooked_timestep as _,
                )
                .expect("Failed to create timestep hook"),
            )
        })
    });
}

fn get_orig_timestep_func() -> Option<BasicNothingFunc> {
    *ORIG_TIMESTEP_FUNC.get().unwrap_or(&None)
}
