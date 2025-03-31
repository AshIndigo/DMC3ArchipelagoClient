use minhook::MinHook;
use std::cell::{RefCell};
use std::ops::{Deref, DerefMut};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs, path, thread};
use std::io::BufReader;
use std::sync::OnceLock;
use imgui_sys::{ImGuiCond, ImGuiCond_Appearing, ImGuiWindowFlags, ImVec2};
use serde::Deserialize;
use hook::CONNECTED;
use crate::{archipelago, hook, utilities};
use crate::archipelago::ArchipelagoData;
use crate::imgui_bindings::*;
use crate::ui::ArchipelagoHud;
use crate::utilities::get_mary_base_address;

thread_local! {
    static HUD_INSTANCE: RefCell<ArchipelagoHud> = RefCell::new(ArchipelagoHud::new());
    //static ORIG_FUNC: Cell<Option<DdmkMainType >> = Cell::new(None);
}
// static mut ORIG_RENDER_FUNC: Cell<Option<BasicNothingFunc>> = Cell::new(None);
// static mut ORIG_TIMESTEP_FUNC: Cell<Option<BasicNothingFunc>> = Cell::new(None);
static SETUP: AtomicBool = AtomicBool::new(false);

const MAIN_FUNC_ADDR: usize = 0xC65E0; // 0xC17B0 (For 2022 ddmk)
const TIMESTEP_FUNC_ADDR: usize = 0x1DE20; // 0x1DC50 (For 2022 ddmk)

unsafe extern "C" fn hooked_timestep() {
    if !SETUP.load(Ordering::SeqCst) {
        MinHook::enable_hook((get_mary_base_address() + MAIN_FUNC_ADDR) as _).expect("Failed to enable hook");
        SETUP.store(true, Ordering::SeqCst);
        if path::Path::new("login_data.json").exists() {
            match fs::File::open("login_data.json") {
                Ok(login_data_file) => {
                    let reader = BufReader::new(login_data_file);
                    let mut json_reader = serde_json::Deserializer::from_reader(reader);
                    match ArchipelagoData::deserialize(&mut json_reader) {
                        Ok(data) => {
                            HUD_INSTANCE.with(|instance| {
                                let mut instance = instance.borrow_mut();
                                instance.deref_mut().arch_url = data.url.to_string();
                                instance.deref_mut().username = data.name.to_string();
                            });
                        },
                        Err(err) => {
                            log::error!("{}", err);
                        }
                    }
                }
                Err(err) => { log::error!("Failed to open login_data.json: {}", err);}
            }
        }

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

unsafe extern "C" fn hooked_render() {
    if !SETUP.load(Ordering::SeqCst) {
        return;
    }
    HUD_INSTANCE.with(|instance| {
        if !utilities::read_bool_from_address_ddmk(0x12c73a) {
            return;
        }
        archipelago_window(instance); // For the archipelago window
        tracking_window();
        match get_orig_render_func() {
            None => {}
            Some(fnc) => {
                fnc();
            }
        }
    })
}

unsafe fn tracking_window() {
    let flag = &mut true;
    get_imgui_pos()(&ImVec2 {
        x: 800.0,
        y: 100.0
    }, ImGuiCond_Appearing as ImGuiCond, &ImVec2 {
        x: 0.0,
        y: 0.0
    });
    get_imgui_begin()("Tracker\0".as_ptr() as *const c_char, flag as *mut bool, imgui_sys::ImGuiWindowFlags_AlwaysAutoResize as ImGuiWindowFlags);
    text("Connection:\0");
    get_imgui_end()();
}

unsafe fn archipelago_window(instance_cell: &RefCell<ArchipelagoHud>) {
    let flag = &mut true;
    let mut instance = instance_cell.borrow_mut();
    get_imgui_pos()(&ImVec2 {
        x: 800.0,
        y: 100.0
    }, ImGuiCond_Appearing as ImGuiCond, &ImVec2 {
        x: 0.0,
        y: 0.0
    });
    get_imgui_begin()("Archipelago\0".as_ptr() as *const c_char, flag as *mut bool, imgui_sys::ImGuiWindowFlags_AlwaysAutoResize as ImGuiWindowFlags);
    text(format!("Status: {:#}\0", if CONNECTED.load(Ordering::SeqCst) { "Connected" } else { "Disconnected" }));
    text("Connection:\0");
    input_rs("URL\0", &mut instance.deref_mut().arch_url, false); // TODO: Slight issue where some letters arent being cleared properly?
    input_rs("Username\0", &mut instance.deref_mut().username, false);
    input_rs("Password\0", &mut instance.deref_mut().password, true);
    if get_imgui_button()("Connect".as_ptr() as *const c_char, &ImVec2 {
        x: 0.0,
        y: 0.0,
    }) {
        log::debug!("Given URL: {}", &mut instance.deref_mut().arch_url.trim().to_string());
        let url = instance.deref().arch_url.clone().trim().to_string();
        let name = instance.deref().username.clone().trim().to_string();
        let password = instance.deref().password.clone().trim().to_string();
        thread::spawn(move || {
            connect_button_pressed(url, name, password);
        });
    }
    get_imgui_end()();
}

#[tokio::main(flavor = "current_thread")]
async fn connect_button_pressed(url: String, name: String, password: String) {
    match archipelago::TX_ARCH.get() {
        None => log::error!("Connect TX doesn't exist"),
        Some(tx) => {
            tx.send(ArchipelagoData { url, name, password, }).expect("Failed to send data");
        }
    }
}

pub unsafe fn setup_ddmk_hook() {
    log::info!("Starting up DDMK hook");
    // let orig_main = get_mary_base_address() + MAIN_FUNC_ADDR;
    //let orig_main = get_mary_base_address() + 0xC17B0; // I think this is main() in DDMK? For 2022 DDMK
    //let orig_timestep = get_mary_base_address() + TIMESTEP_FUNC_ADDR;
    log::info!("Mary base ADDR: {}", get_mary_base_address());
    init_render_func();
    init_timestep_func();
    //ORIG_RENDER_FUNC.set(Some(std::mem::transmute::<_, BasicNothingFunc>(MinHook::create_hook(orig_main as _, hooked_render as _).expect("Failed to create hook"))));
    //ORIG_TIMESTEP_FUNC.set(Some(std::mem::transmute::<_, BasicNothingFunc>(MinHook::create_hook(orig_timestep as _, hooked_timestep as _).expect("Failed to create timestep hook"))));
    MinHook::enable_hook((get_mary_base_address() + TIMESTEP_FUNC_ADDR) as _).expect("Failed to enable timestep hook");
    log::info!("DDMK hook initialized");
}

static ORIG_RENDER_FUNC: OnceLock<Option<BasicNothingFunc>> = OnceLock::new();

fn init_render_func() {
    ORIG_RENDER_FUNC.get_or_init(|| {
        Some(unsafe {
            std::mem::transmute::<_, BasicNothingFunc>(
                MinHook::create_hook((get_mary_base_address() + MAIN_FUNC_ADDR) as _, hooked_render as _)
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
                MinHook::create_hook((get_mary_base_address() + TIMESTEP_FUNC_ADDR) as _, hooked_timestep as _)
                    .expect("Failed to create timestep hook"),
            )
        })
    });
}

fn get_orig_timestep_func() -> Option<BasicNothingFunc> {
    *ORIG_TIMESTEP_FUNC.get().unwrap_or(&None)
}