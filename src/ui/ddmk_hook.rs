use crate::constants::ItemCategory;
use crate::ui::imgui_bindings::*;
use crate::ui::ui;
use crate::ui::ui::{get_status_text, ArchipelagoHud, CHECKLIST};
use crate::utilities::get_mary_base_address;
use crate::{bank, constants, utilities};
use imgui_sys::{ImGuiCond, ImGuiCond_Always, ImGuiCond_Appearing, ImGuiWindowFlags, ImVec2};
use minhook::MinHook;
use std::ffi::c_int;
use std::ops::DerefMut;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{MutexGuard, OnceLock};
use std::thread;

static SETUP: AtomicBool = AtomicBool::new(false);

const MAIN_FUNC_ADDR: usize = 0xC65E0; // 0xC17B0 (For 2022 ddmk)
const TIMESTEP_FUNC_ADDR: usize = 0x1DE20; // 0x1DC50 (For 2022 ddmk)
const DDMK_UI_ENABLED: usize = 0x12c73a;

unsafe extern "C" fn hooked_timestep() {
    unsafe {
        if !SETUP.load(Ordering::SeqCst) {
            MinHook::enable_hook((get_mary_base_address() + MAIN_FUNC_ADDR) as _)
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
        match ui::get_hud_data().lock() {
            Ok(instance) => {
                if false {
                    // TODO: Hud for pause menu and main menu
                    on_screen_hud();
                }
                if !utilities::read_bool_from_address_ddmk(DDMK_UI_ENABLED) {
                    return;
                }
                archipelago_window(instance); // For the archipelago window
                tracking_window();
                bank_window();
                match get_orig_render_func() {
                    None => {}
                    Some(fnc) => {
                        fnc();
                    }
                }
            }
            Err(err) => {
                log::error!("{}", err);
            }
        }
    }
}

unsafe fn tracking_window() {
    unsafe {
        let flag = &mut true;
        get_imgui_next_pos()(
            &ImVec2 { x: 800.0, y: 300.0 },
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
                bank::get_bank().lock().unwrap().get(item).unwrap()
            ));
            get_imgui_same_line()(0f32, 5f32); // TODO Figure out how to align properly
            get_imgui_push_id()(n as c_int);
            if get_imgui_button()(
                "Retrieve 1\0".as_ptr() as *const c_char,
                &ImVec2 { x: 0.0, y: 0.0 },
            ) {
                if bank::can_add_item_to_current_inv(item) {
                    ui::retrieve_button_pressed(item);
                }
            }
            get_imgui_pop_id()();
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

pub unsafe fn archipelago_window(mut instance: MutexGuard<ArchipelagoHud>) {
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
        text("Connection:\0");
        input_rs("URL\0", &mut instance.archipelago_url, false); // TODO: Slight issue where some letters arent being cleared properly?
        input_rs("Username\0", &mut instance.username, false);
        input_rs("Password\0", &mut instance.password, true);
        if get_imgui_button()(
            "Connect\0".as_ptr() as *const c_char,
            &ImVec2 { x: 0.0, y: 0.0 },
        ) {
            log::debug!(
                "Given URL: {}\0",
                &mut instance.deref_mut().archipelago_url.trim().to_string()
            );
            ui::connect_button_pressed(
                instance.archipelago_url.clone().trim().to_string(),
                instance.username.clone().trim().to_string(),
                instance.password.clone().trim().to_string(),
            );
        }

        if get_imgui_button()(
            "Display Message\0".as_ptr() as *const c_char,
            &ImVec2 { x: 0.0, y: 0.0 },
        ) {
            thread::spawn(move || {
                utilities::display_message(&"Test Message".to_string());
            });
        }
        get_imgui_end()();
    }
}

pub fn setup_ddmk_hook() {
    log::info!("Starting up DDMK hook");
    log::info!("Mary base ADDR: {:x}", get_mary_base_address());
    init_render_func();
    init_timestep_func();
    unsafe {
        MinHook::enable_hook((get_mary_base_address() + TIMESTEP_FUNC_ADDR) as _)
            .expect("Failed to enable timestep hook");
    }
    log::info!("DDMK hook initialized");
}

static ORIG_RENDER_FUNC: OnceLock<Option<BasicNothingFunc>> = OnceLock::new();

fn init_render_func() {
    ORIG_RENDER_FUNC.get_or_init(|| {
        Some(unsafe {
            std::mem::transmute::<_, BasicNothingFunc>(
                MinHook::create_hook(
                    (get_mary_base_address() + MAIN_FUNC_ADDR) as _,
                    hooked_render as _,
                )
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
                    (get_mary_base_address() + TIMESTEP_FUNC_ADDR) as _,
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
