use crate::archipelago::CONNECTED;
use crate::constants::ItemCategory;
use crate::ui::text_handler;
use crate::utilities::read_data_from_address;
use crate::{check_handler, config, constants, game_manager};
use imgui_sys::{ImGuiCond, ImGuiCond_Appearing, ImGuiWindowFlags, ImVec2};
use randomizer_utilities::dmc::common_ddmk;
use randomizer_utilities::dmc::common_ddmk::{SETUP, checkbox_text};
use randomizer_utilities::dmc::dmc_constants::DDMKHandler;
use randomizer_utilities::get_base_address;
use std::os::raw::c_char;
use std::sync::LazyLock;
use std::sync::atomic::Ordering;
use std::thread;

pub static MARY_ADDRESS: LazyLock<usize> = LazyLock::new(|| get_base_address("Mary.dll"));

pub const USE_2022_DDMK: bool = true;
const DDMK_UI_ENABLED: usize = if USE_2022_DDMK { 0x1258da } else { 0x13374a }; //0x12c73a; (Probably old indigo ver.)

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
        match common_ddmk::get_orig_render_func() {
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
        common_ddmk::get_imgui_next_pos()(
            &ImVec2 { x: 800.0, y: 320.0 }, // 300
            ImGuiCond_Appearing as ImGuiCond,
            &ImVec2 { x: 0.0, y: 0.0 },
        );
        common_ddmk::get_imgui_begin()(
            c"Tracker".as_ptr() as *const c_char,
            flag as *mut bool,
            imgui_sys::ImGuiWindowFlags_AlwaysAutoResize as ImGuiWindowFlags,
        );

        match game_manager::ARCHIPELAGO_DATA.read() {
            Ok(data) => {
                for chunk in constants::get_items_by_category(ItemCategory::Key).chunks(3) {
                    let row_text = chunk
                        .iter()
                        .map(|&item| checkbox_text(&item.to_string(), &data.items))
                        .collect::<Vec<String>>()
                        .join("  ");
                    common_ddmk::text(format!("{}\0", row_text));
                }
                common_ddmk::text(format!("Blue Orbs: {}\0", data.blue_orbs));
                common_ddmk::text(format!("Purple Orbs: {}\0", data.purple_orbs));
            }
            Err(err) => {
                log::error!("Failed to read ArchipelagoData: {:?}", err);
            }
        }

        common_ddmk::get_imgui_end()();
    }
}

pub unsafe fn archipelago_window() {
    unsafe {
        let flag = &mut true;
        common_ddmk::get_imgui_next_pos()(
            &ImVec2 { x: 800.0, y: 100.0 },
            ImGuiCond_Appearing as ImGuiCond,
            &ImVec2 { x: 0.0, y: 0.0 },
        );
        common_ddmk::get_imgui_begin()(
            c"Archipelago".as_ptr() as *const c_char,
            flag as *mut bool,
            imgui_sys::ImGuiWindowFlags_AlwaysAutoResize as ImGuiWindowFlags,
        );
        common_ddmk::text(format!(
            "Status: {}\0",
            if CONNECTED.load(Ordering::SeqCst) {
                "Connected"
            } else {
                "Disconnected"
            }
        ));
        const DEBUG: bool = false;
        if DEBUG {
            if common_ddmk::get_imgui_button()(
                c"Clear dummy flags".as_ptr() as *const c_char,
                &ImVec2 { x: 0.0, y: 0.0 },
            ) {
                thread::spawn(move || {
                    check_handler::clear_high_roller();
                });
            }
            if common_ddmk::get_imgui_button()(
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
            if common_ddmk::get_imgui_button()(
                c"Edit Message Index".as_ptr() as *const c_char,
                &ImVec2 { x: 0.0, y: 0.0 },
            ) {
                thread::spawn(move || {
                    text_handler::display_message_via_index("Testing test");
                });
            }
        }
        common_ddmk::get_imgui_end()();
    }
}

pub fn setup_ddmk_hook() {
    if !config::CONFIG.mods.disable_ddmk_hooks {
        log::info!("Starting up DDMK hook");
        log::info!("Mary base ADDR: {:X}", *MARY_ADDRESS);
        if common_ddmk::DDMK_INFO
            .set(DDMKHandler {
                ddmk_address: LazyLock::new(|| get_base_address("Mary.dll")),
                main_func_addr: 0xC17B0,
                timestep_func_addr: 0x1DC50,
                ddmk_ui_enabled: DDMK_UI_ENABLED,
                hooked_render: hooked_render as _,
                text_addr: 0x65210,
                end_addr: 0x24cd0,
                begin_addr: 0x1f640,
                button_addr: 0x55920,
                next_pos: 0x351b0,
            })
            .is_err()
        {
            log::error!("Failed to set DDMK info");
        }
        common_ddmk::run_common_ddmk_code();
        log::info!("DDMK hook initialized");
    } else {
        log::info!("DDMK is detected but hooks will not be enabled")
    }
}
