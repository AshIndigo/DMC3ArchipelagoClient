use imgui::Context;
use minhook::MinHook;
use std::cell::{Cell, RefCell};
use std::ffi::OsStr;
use std::ops::{Deref, DerefMut};
use std::os::raw::c_char;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use imgui_sys::ImVec2;
use winapi::shared::minwindef::HINSTANCE;
use winapi::um::libloaderapi::GetModuleHandleW;
use hook::CONNECTED;
use crate::{archipelago, hook};
use crate::archipelago::{ArchipelagoData, CONNECT_CHANNEL_SETUP};
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
        Some(timestep_func) => {
            timestep_func();
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
        let flag = &mut true;
        if !read_bool_from_address(0x12c73a) {
            return;
        }
        std::mem::transmute::<_, ImGuiBegin>(get_mary_base_address() + BEGIN_FUNC_ADDR)("Archipelago\0".as_ptr() as *const c_char, flag as *mut bool, 0);
        text(format!("Status: {:#}\0", CONNECTED.load(Ordering::SeqCst)));
        text("Connection:\0");
        input_rs("URL\0", &mut instance.deref_mut().arch_url, false); // TODO: Slight issue where some letters arent being cleared properly?
        input_rs("Username\0", &mut instance.deref_mut().username, false);
        input_rs("Password\0", &mut instance.deref_mut().password, true);
        if std::mem::transmute::<_, ImGuiButton>(get_mary_base_address() + BUTTON_ADDR)("Connect".as_ptr() as *const c_char, &ImVec2 {
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
        std::mem::transmute::<_, BasicNothingFunc>(get_mary_base_address() + END_FUNC_ADDR)();
        match ORIG_RENDER_FUNC.get() {
            None => {}
            Some(fnc) => {
                fnc();
            }
        }
    })
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

type BasicNothingFunc = unsafe extern "system" fn(); // No args no returns

pub unsafe extern "system" fn get_mary_base_address() -> usize {
    crate::hook::get_base_address("Mary.dll")
}

pub unsafe fn setup_ddmk_hook() {
    log::info!("Starting up hook");
    let orig_main = get_mary_base_address() + MAIN_FUNC_ADDR;
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