#![feature(lock_value_accessors)]
#![recursion_limit = "512"]

use crate::ddmk_hook::setup_ddmk_hook;
use crate::hook::create_console;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::ffi::c_void;
use std::sync::OnceLock;
use std::thread;
use winapi::shared::guiddef::REFIID;
use winapi::shared::minwindef::{DWORD, LPVOID};
use winapi::um::libloaderapi::LoadLibraryA;
use winapi::um::winnt::HRESULT;
use windows::core::BOOL;
use windows::Win32::Foundation::*;

mod hook;
mod cache;
mod archipelago;
mod generated_locations;
mod constants;
mod config;
mod ddmk_hook;
mod ui;
mod imgui_bindings;
mod inputs;
mod asm_hook;
mod utilities;
mod experiments;

static mut REAL_DIRECTINPUT8CREATE: Option<
    unsafe extern "system" fn(HINSTANCE, DWORD, REFIID, *mut *mut c_void, *mut c_void) -> HRESULT
> = None;

fn load_real_dinput8() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let system_path = std::env::var("WINDIR").unwrap_or("C:\\Windows".to_string());
        let real_path = format!("{system_path}\\System32\\dinput8.dll");

        let lib = unsafe { libloading::Library::new(&real_path) }.expect("Failed to load real dinput8");

        unsafe {
            REAL_DIRECTINPUT8CREATE = Some(
                *lib.get::<unsafe extern "system" fn(HINSTANCE, DWORD, REFIID, *mut *mut c_void, *mut c_void) -> HRESULT>(b"DirectInput8Create\0").unwrap()
            );
            std::mem::forget(lib); // Don't drop it, keep loaded
        }
    });
}

pub static DLL_HINST: OnceLock<isize> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "system" fn DllMain(
    _hinst_dll: HINSTANCE,
    fdw_reason: u32,
    _lpv_reserved: LPVOID,
) -> BOOL {
    const DLL_PROCESS_ATTACH: u32 = 1;
    const DLL_PROCESS_DETACH: u32 = 0;
    const DLL_THREAD_ATTACH: u32 = 2;
    const DLL_THREAD_DETACH: u32 = 3;

    match fdw_reason {
        DLL_PROCESS_ATTACH => {
            DLL_HINST.set(_hinst_dll.0 as isize).expect("Failed to set hinst dll");
            thread::spawn(|| {
                load_real_dinput8();
                load_other_dlls();
                main_setup();
            });
        },
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

fn load_other_dlls() {
    let _ = unsafe { LoadLibraryA(b"Mary.dll\0".as_ptr() as _) };
}


fn main_setup() {
    SimpleLogger::new()
        .with_module_level("tokio", LevelFilter::Warn)
        .with_module_level("tungstenite::protocol", LevelFilter::Warn)
        .with_module_level("hudhook::hooks::dx11", LevelFilter::Warn)
        .with_module_level("tracing::span", LevelFilter::Warn)
        .with_module_level("winit::window", LevelFilter::Warn)
        .with_module_level("eframe::native::run", LevelFilter::Warn)
        .with_module_level("eframe::native::glow_integration", LevelFilter::Warn)
        .with_threads(true)
        .init()
        .unwrap();
    create_console();
    let rx = hook::setup_items_channel();
    if utilities::is_ddmk_loaded() {
        log::info!("DDMK is loaded!");
        setup_ddmk_hook();
    } else {
        log::info!("DDMK is not loaded!");
        //experiments::egui::start_egui();
        // thread::Builder::new()
        //     .name("Archipelago HUD".to_string())
        //     .spawn(move || {
        //         hudhook_hook::start_imgui_hudhook(); // HudHook wants to be in its own thread
        //     }).expect("Failed to spawn ui thread");
    }
    thread::Builder::new()
        .name("Archipelago Client".to_string())
        .spawn(move || {
            hook::spawn_arch_thread(rx);
        })
        .expect("Failed to spawn arch thread");
    hook::install_initial_functions(); // Need to run this when actually connecting?
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn DirectInput8Create(
    hinst: HINSTANCE,
    dwVersion: DWORD,
    riidltf: REFIID,
    ppvOut: *mut *mut c_void,
    punkOuter: *mut c_void,
) -> HRESULT {
    unsafe {
        // call into the real dinput8.dll
        load_real_dinput8(); // lazy-load if needed
        REAL_DIRECTINPUT8CREATE.expect("not loaded")(
            hinst, dwVersion, riidltf, ppvOut, punkOuter
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // let result = add(2, 2);
        // assert_eq!(result, 4);
    }
}
