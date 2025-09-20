use crate::constants::ItemCategory;
use crate::item_sync::{BLUE_ORBS_OBTAINED, PURPLE_ORBS_OBTAINED};
use crate::ui::imgui_bindings::*;
use crate::ui::ui::{get_status_text, CHECKLIST};
use crate::utilities::read_data_from_address;
use crate::{bank, check_handler, config, constants, game_manager, text_handler, utilities};
use imgui_sys::{ImGuiCond, ImGuiCond_Always, ImGuiCond_Appearing, ImGuiWindowFlags, ImVec2};
use minhook::MinHook;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::thread;
use std::time::Duration;

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
        if false {
            // TODO: Hud for pause menu and main menu
            on_screen_hud();
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
            "Tracker\0".as_ptr() as *const c_char,
            flag as *mut bool,
            imgui_sys::ImGuiWindowFlags_AlwaysAutoResize as ImGuiWindowFlags,
        );
        for chunk in constants::get_items_by_category(ItemCategory::Key).chunks(3) {
            let row_text = chunk
                .iter()
                .map(|&item| checkbox_text(item))
                .collect::<Vec<String>>()
                .join("  "); // TODO Pretty this up later
            text(format!("{}\0", row_text));
        }
        text(format!(
            "Blue Orbs: {}\0",
            BLUE_ORBS_OBTAINED.load(Ordering::SeqCst)
        ));
        text(format!(
            "Purple Orbs: {}\0",
            PURPLE_ORBS_OBTAINED.load(Ordering::SeqCst)
        ));
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
            "Bank\0".as_ptr() as *const c_char,
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

fn checkbox_text(item: &str) -> String {
    let state = CHECKLIST
        .get()
        .and_then(|lock| lock.read().ok())
        .and_then(|map| map.get(item).copied())
        .unwrap_or(false);
    format!("{} [{}]", item, if state { "X" } else { " " })
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
            "Archipelago\0".as_ptr() as *const c_char,
            flag as *mut bool,
            imgui_sys::ImGuiWindowFlags_AlwaysAutoResize as ImGuiWindowFlags,
        );
        text(format!("Status: {}\0", get_status_text()));
        const DEBUG: bool = false;
        if DEBUG {
            if get_imgui_button()(
                "Display Message\0".as_ptr() as *const c_char,
                &ImVec2 { x: 0.0, y: 0.0 },
            ) {
                // thread::spawn(move || event_table_handler::call_event());
            }
            if get_imgui_button()(
                "Display Message New\0".as_ptr() as *const c_char,
                &ImVec2 { x: 0.0, y: 0.0 },
            ) {
                thread::spawn(move || {
                    text_handler::display_text(
                        &"Test Message\x00\x2E".to_string(),
                        Duration::from_secs(5),
                        100,
                        -100,
                    );
                });
            }
            if get_imgui_button()(
                "Clear dummy flags\0".as_ptr() as *const c_char,
                &ImVec2 { x: 0.0, y: 0.0 },
            ) {
                thread::spawn(move || {
                    check_handler::clear_high_roller();
                });
            }
            if get_imgui_button()(
                "Kill Dante\0".as_ptr() as *const c_char,
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
                "Edit Message Index\0".as_ptr() as *const c_char,
                &ImVec2 { x: 0.0, y: 0.0 },
            ) {
                thread::spawn(move || {
                    text_handler::display_message_via_index("Testing test".to_string());
                });
            }
        }
        get_imgui_end()();
    }
}

pub fn setup_ddmk_hook() {
    if !(*config::CONFIG).mods.disable_ddmk_hooks {
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

fn on_screen_hud() {
    get_imgui_next_pos()(
        &ImVec2 { x: 800.0, y: 100.0 },
        ImGuiCond_Always as ImGuiCond,
        &ImVec2 { x: 0.0, y: 0.0 },
    );
    get_imgui_next_size()(
        &ImVec2 { x: 100.0, y: 100.0 },
        ImGuiCond_Always as ImGuiCond,
        &ImVec2 { x: 0.0, y: 0.0 },
    );
    let flag = &mut true;
    get_imgui_begin()(
        "Archipelago\0".as_ptr() as *const c_char,
        flag as *mut bool,
        imgui_sys::ImGuiWindowFlags_AlwaysAutoResize as ImGuiWindowFlags,
    );
    text("test\0");
}
