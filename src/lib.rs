use crate::archipelago::ArchipelagoCore;
use crate::constants::{BasicNothingFunc, DMC3Config};
use crate::utilities::DMC3_ADDRESS;
use crate::utilities::{is_crimson_loaded, is_ddmk_loaded};
use minhook::{MinHook, MH_STATUS};
use randomizer_utilities::exception_handler;
use randomizer_utilities::mapping_utilities::GameConfig;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use windows::core::{BOOL, PCSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader;

mod archipelago;
mod bank;
mod check_handler;
mod compat;
mod config;
mod constants;
mod data;
mod game_manager;
mod hook;
mod location_handler;
mod mapping;
mod save_handler;
mod skill_manager;
mod ui;
mod utilities;

#[macro_export]
/// Does not enable the hook, that needs to be done separately
macro_rules! create_hook {
    ($offset:expr, $detour:expr, $storage:ident, $name:expr) => {{
        let target = (*DMC3_ADDRESS + $offset) as *mut _;
        let detour_ptr = ($detour as *const ()) as *mut std::ffi::c_void;
        let original = MinHook::create_hook(target, detour_ptr)?;
        // This upsets clippy, but oh well
        $storage
            .set(std::mem::transmute::<*mut std::ffi::c_void, _>(original))
            .expect(concat!($name, " hook already set"));
        //log::debug!("{name} hook created", name = $name);
    }};
}

#[derive(Debug)]
#[repr(C)]
pub(crate) struct LoaderStatus {
    // DMC3
    pub dmc3_hash_error: bool,
    pub crimson_hash_error: bool,
    pub ddmk_dmc3_hash_error: bool,
    // DMC1
    pub dmc1_hash_error: bool,
    pub ddmk_dmc1_hash_error: bool,
}

impl Display for LoaderStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

type GetStatusFn = unsafe extern "C" fn() -> *const LoaderStatus;

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(
    _hinst_dll: HINSTANCE,
    fdw_reason: u32,
    _lpv_reserved: *mut std::os::raw::c_void,
) -> BOOL {
    const DLL_PROCESS_ATTACH: u32 = 1;
    const DLL_PROCESS_DETACH: u32 = 0;
    const DLL_THREAD_ATTACH: u32 = 2;
    const DLL_THREAD_DETACH: u32 = 3;

    match fdw_reason {
        DLL_PROCESS_ATTACH => {
            ui::dx11_hooks::setup_overlay();
            randomizer_utilities::setup_logger("dmc3_randomizer");
            // Loader status
            thread::spawn(|| unsafe {
                let loader_hmodule = LibraryLoader::LoadLibraryA(PCSTR::from_raw(
                    c"dinput8.dll".as_ptr() as *const u8,
                ));
                let proc_addr = LibraryLoader::GetProcAddress(
                    loader_hmodule.unwrap(),
                    PCSTR::from_raw(c"get_loader_status".as_ptr() as *const u8),
                );
                // TODO Make this display on the overlay
                let loader_status = &*std::mem::transmute::<FARPROC, GetStatusFn>(proc_addr)();
                log::info!("Loader Status: {loader_status:?}");
            });

            thread::spawn(|| {
                main_setup();
            });
        }
        DLL_PROCESS_DETACH => {
            // For cleanup
        }
        DLL_THREAD_ATTACH | DLL_THREAD_DETACH => {
            // Normally ignored if DisableThreadLibraryCalls is used
        }
        _ => {}
    }

    BOOL(1)
}

fn setup_main_loop_hook() -> Result<(), MH_STATUS> {
    unsafe {
        create_hook!(
            MAIN_LOOP_ADDR,
            main_loop_hook,
            MAIN_LOOP_ORIGINAL,
            "Main loop hook"
        );
        MinHook::enable_hook((*DMC3_ADDRESS + MAIN_LOOP_ADDR) as *mut _)?;
    }
    Ok(())
}

pub static AP_CORE: OnceLock<Arc<Mutex<ArchipelagoCore>>> = OnceLock::new();

static MAIN_LOOP_ORIGINAL: OnceLock<BasicNothingFunc> = OnceLock::new();
const MAIN_LOOP_ADDR: usize = 0x337df0;
fn main_loop_hook() {
    // Run original game code
    if let Some(func) = MAIN_LOOP_ORIGINAL.get() {
        unsafe {
            func();
        }
    }

    if let Err(err) = AP_CORE
        .get_or_init(|| {
            ArchipelagoCore::new(
                config::CONFIG.connections.get_url(),
                DMC3Config::GAME_NAME.parse().unwrap(),
            )
            .map(|core| Arc::new(Mutex::new(core)))
            .unwrap()
        })
        .lock()
        .unwrap()
        .update()
    {
        log::error!("{}", err);
    }
}

fn main_setup() {
    exception_handler::install_exception_handler();
    if is_ddmk_loaded() {
        log::info!("DDMK is loaded!");
        log::warn!(
            "DDMK's Actor system most likely does not work with the DeathLink setting in the randomizer, \
                please turn it off if you wish to use DeathLink"
        );
        compat::ddmk_hook::setup_ddmk_hook();
    } else if is_crimson_loaded() {
        log::info!("Crimson is loaded!");
        log::warn!(
            "Crimson's Crimson/Style switcher mode does not work with the DeathLink setting in the randomizer, \
                please turn it off if you wish to use DeathLink"
        );
        compat::crimson_hook::setup_crimson_hook();
    } else {
        log::info!("DDMK or Crimson are not loaded!");
    }
    log::info!("DMC3 Base Address is: {:X}", *DMC3_ADDRESS);
    setup_main_loop_hook().unwrap();
}
