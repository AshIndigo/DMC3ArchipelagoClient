#![feature(lock_value_accessors)]
#![feature(ascii_char)]
#![recursion_limit = "512"]

use crate::hook::create_console;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::ffi::c_void;
use std::sync::OnceLock;
use std::thread;
use winapi::shared::guiddef::REFIID;
use winapi::shared::minwindef::{DWORD, LPVOID};
use winapi::um::errhandlingapi::AddVectoredExceptionHandler;
use winapi::um::libloaderapi::LoadLibraryA;
use winapi::um::winnt::{EXCEPTION_POINTERS, HRESULT};
use windows::Win32::Foundation::*;
use windows::core::BOOL;
use windows::Win32::System::Diagnostics::Debug::EXCEPTION_CONTINUE_SEARCH;

mod archipelago;
mod cache;
mod constants;
mod experiments;
mod generated_locations;
mod hook;
mod ui;
mod utilities;
mod save_handler;
mod check_handler;
mod bank;

static mut REAL_DIRECTINPUT8CREATE: Option<
    unsafe extern "system" fn(HINSTANCE, DWORD, REFIID, *mut *mut c_void, *mut c_void) -> HRESULT,
> = None;

fn load_real_dinput8() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let system_path = std::env::var("WINDIR").unwrap_or("C:\\Windows".to_string());
        let real_path = format!("{system_path}\\System32\\dinput8.dll");

        let lib =
            unsafe { libloading::Library::new(&real_path) }.expect("Failed to load real dinput8");

        unsafe {
            REAL_DIRECTINPUT8CREATE = Some(
                *lib.get::<unsafe extern "system" fn(
                    HINSTANCE,
                    DWORD,
                    REFIID,
                    *mut *mut c_void,
                    *mut c_void,
                ) -> HRESULT>(b"DirectInput8Create\0")
                    .unwrap(),
            );
            std::mem::forget(lib); // Don't drop it, keep loaded
        }
    });
}

pub static DLL_HINST: OnceLock<isize> = OnceLock::new();

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
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
            DLL_HINST
                .set(_hinst_dll.0 as isize)
                .expect("Failed to set hinst dll");
            thread::spawn(|| {
                load_real_dinput8();
                load_other_dlls();
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

extern "system" fn exception_handler(exception_info: *mut EXCEPTION_POINTERS) -> i32 {
    unsafe {
        let record = &*(*exception_info).ExceptionRecord;
        let code = record.ExceptionCode;

        if code == EXCEPTION_ACCESS_VIOLATION.0 as u32
            || code == EXCEPTION_ILLEGAL_INSTRUCTION.0 as u32
            || code == EXCEPTION_INT_DIVIDE_BY_ZERO.0 as u32
        {
            log::error!("Caught exception: {:X}", code);
            log::error!("Address: {:?}", (*exception_info).ContextRecord);
        }
    }
    EXCEPTION_CONTINUE_SEARCH
}

/*unsafe fn log_stack_trace(ctx: *mut CONTEXT) {
    let process = GetCurrentProcess();
    let thread = GetCurrentThread();

    if SymInitialize(process, None, true).is_err() {
        log::error!("SymInitialize failed");
        return;
    }

    SymSetOptions(SYMOPT_LOAD_LINES | SYMOPT_UNDNAME);

    let mut stack: STACKFRAME64 = zeroed();
    stack.AddrPC.Offset = (*ctx).Rip;
    stack.AddrPC.Mode = AddrModeFlat;
    stack.AddrFrame.Offset = (*ctx).Rbp;
    stack.AddrFrame.Mode = AddrModeFlat;
    stack.AddrStack.Offset = (*ctx).Rsp;
    stack.AddrStack.Mode = AddrModeFlat;

    const MAX_FRAMES: usize = 64;
    for _ in 0..MAX_FRAMES {
        // let backtrace = Backtrace::capture();
        // // let ok = StackWalk64(
        // //     IMAGE_FILE_MACHINE_AMD64 as u32,
        // //     process,
        // //     thread,
        // //     &mut stack,
        // //     ctx as *mut _,
        // //     None,
        // //     Some(func_table_access),
        // //     Some(module_base),
        // //     None,
        // // );
        // 
        // if ok == FALSE || stack.AddrPC.Offset == 0 {
        //     break;
        // }

        let addr = stack.AddrPC.Offset;

        let mut buffer = [0u8; size_of::<SYMBOL_INFO>() + 256];
        let sym_info = buffer.as_mut_ptr() as *mut SYMBOL_INFO;
        (*sym_info).SizeOfStruct = size_of::<SYMBOL_INFO>() as u32;
        (*sym_info).MaxNameLen = 255;

        unsafe {
            if SymFromAddr(process, addr, None, sym_info).is_ok() {
                let name = CStr::from_ptr((*sym_info).Name.as_ptr());
                let name_str = name.to_string_lossy();
                log::error!("📦 Other: {} - 0x{:X}", name_str, addr);
            } else {
                log::error!("❓ Unknown - 0x{:X}", addr);
            }
        }
    }

    unsafe { SymCleanup(process).expect("Failed to Sym Cleanup"); }
}*/

fn install_exception_handler() {
    unsafe {
        AddVectoredExceptionHandler(1, Some(exception_handler));
    }
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
    install_exception_handler();
    if utilities::is_ddmk_loaded() {
        log::info!("DDMK is loaded!");
        ui::ddmk_hook::setup_ddmk_hook();
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
            hook::spawn_arch_thread();
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
        REAL_DIRECTINPUT8CREATE.expect("not loaded")(hinst, dwVersion, riidltf, ppvOut, punkOuter)
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
