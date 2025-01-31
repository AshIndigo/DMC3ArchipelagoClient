use imgui::sys::{cty, ImGuiContext, ImGuiWindowFlags};
use imgui::{sys, Context, FontSource};
use minhook::MinHook;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::ops::{Add, Deref, DerefMut};
use std::os::raw::c_char;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use anyhow::Error;
use archipelago_rs::client::ArchipelagoClient;
use imgui_sys::ImVec2;
use winapi::shared::minwindef::HINSTANCE;
use winapi::um::libloaderapi::GetModuleHandleW;
use crate::archipelago::connect_archipelago;
use crate::hook::get_dmc3_base_address;
use crate::imgui_bindings;
use crate::imgui_bindings::{*};
use crate::ui::ArchipelagoHud;

thread_local! {
    static HUD_INSTANCE: RefCell<ArchipelagoHud> = RefCell::new(ArchipelagoHud::new());
    //static ORIG_FUNC: Cell<Option<DdmkMainType >> = Cell::new(None);
}
static mut ORIG_RENDER_FUNC: Cell<Option<BasicNothingFunc>> = Cell::new(None);
static mut ORIG_TIMESTEP_FUNC: Cell<Option<BasicNothingFunc>> = Cell::new(None);
static mut CTX: RefCell<Option<Context>> = RefCell::new(None);
static SETUP: AtomicBool = AtomicBool::new(false);

const MAIN_FUNC_ADDR: usize = 0xC65E0; // 0xC17B0 (For 2022 ddmk)
const TIMESTEP_FUNC_ADDR: usize = 0x1DE20; // 0x1DC50 (For 2022 ddmk)

unsafe extern "C" fn hooked_timestep() {
    if !SETUP.load(Ordering::SeqCst) {
        MinHook::enable_hook((get_mary_base_address() + MAIN_FUNC_ADDR) as _).expect("Failed to enable hook");
        SETUP.store(true, Ordering::SeqCst);
    }
    match ORIG_TIMESTEP_FUNC.get() {
        None => {
            panic!("ORIG_TIMESTEP_FUNC not initialized in hooked render");
        }
        Some(fnc) => {
            fnc();
            //log::info!("Ran orig timestep function");
        }
    }
}

fn read_bool_from_address(address: usize) -> bool {
    unsafe { *((address + get_mary_base_address()) as *const bool) }
}

unsafe extern "C" fn hooked_render() {
    if !SETUP.load(Ordering::SeqCst) {
        return;
    }
    HUD_INSTANCE.with(|instance| {
        let mut instance = instance.borrow_mut();
        let mut flag = &mut true;
        if !read_bool_from_address(0x12c73a) {
            return;
        }
        std::mem::transmute::<_, ImGuiBegin>(get_mary_base_address() + BEGIN_FUNC_ADDR)("Archipelago".as_ptr() as *const c_char, flag as *mut bool, 0);
        text("Connection:");
        input_text("Archipelago URL", &mut instance.deref_mut().arch_url); // TODO: Slight issue where some letters arent being cleared properly?
        input_text("Archipelago Username", &mut instance.deref_mut().username);
        input_text("Archipelago Password", &mut instance.deref_mut().password);
        if std::mem::transmute::<_, ImGuiButton>(get_mary_base_address() + BUTTON_ADDR)("Connect".as_ptr() as *const c_char, &ImVec2 {
            x: 0.0,
            y: 0.0,
        }) {
            log::debug!("Given URL: {}", &mut instance.deref_mut().arch_url);
            let url = instance.deref().arch_url.clone();
            let name = instance.deref().username.clone();
            let password = instance.deref().password.clone();
            thread::spawn(async || {
                match connect_archipelago(url, name, password).await {
                    Ok(_) => {
                        log::info!("Archipelago Connected")
                    }
                    Err(err) => { log::error!("Failed to connect to archipelago: {}", err); }
                }
            });
        }
        std::mem::transmute::<_, BasicNothingFunc>(get_mary_base_address() + END_FUNC_ADDR)();
        match ORIG_RENDER_FUNC.get() {
            None => {}
            Some(fnc) => {
                fnc();
            }
        }
    })
}

type BasicNothingFunc = unsafe extern "system" fn(); // No args no returns

pub unsafe extern "system" fn get_mary_base_address() -> usize {
    let wide_name: Vec<u16> = OsStr::new("Mary.dll")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let module_handle: HINSTANCE = GetModuleHandleW(wide_name.as_ptr());
        if !module_handle.is_null() {
            module_handle as *mut _ as usize
        } else {
            0
        }
    }
}

pub unsafe fn setup_ddmk_hook() {
    log::info!("Starting up hook");
    let orig_main = get_mary_base_address() + MAIN_FUNC_ADDR;
    let mary_begin = get_mary_base_address() + BEGIN_FUNC_ADDR;
    let mary_end = get_mary_base_address() + END_FUNC_ADDR;
    //let orig_main = get_mary_base_address() + 0xC17B0; // I think this is main() in DDMK? For 2022 DDMK
    let orig_timestep = get_mary_base_address() + TIMESTEP_FUNC_ADDR;
    // ORIG_FUNC.set(Some(std::mem::transmute::<_, DdmkMainType>(orig_main)));
    // match ORIG_FUNC.get() {
    //     None => {
    //         panic!("ORIG_FUNC not initialized during setup");
    //     }
    //     Some(fnc_ptr) => {
    //         MinHook::create_hook(fnc_ptr as _, hooked_render as _).expect("Failed to create hook");
    //     }
    // }
    log::info!("Mary base ADDR: {}", get_mary_base_address());
    ORIG_RENDER_FUNC.set(Some(std::mem::transmute::<_, BasicNothingFunc>(MinHook::create_hook(orig_main as _, hooked_render as _).expect("Failed to create hook"))));
    ORIG_TIMESTEP_FUNC.set(Some(std::mem::transmute::<_, BasicNothingFunc>(MinHook::create_hook(orig_timestep as _, hooked_timestep as _).expect("Failed to create timestep hook"))));
    //let transmute1 = std::mem::transmute::<_, ImGuiBegin>(get_mary_base_address() + BEGIN_FUNC_ADDR);
    //set_imgui_context();
    MinHook::enable_hook(orig_timestep as _).expect("Failed to enable timestep hook");
    log::info!("DDMK hook initialized");
}