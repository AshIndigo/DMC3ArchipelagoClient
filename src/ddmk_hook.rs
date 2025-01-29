use imgui::sys::{cty, ImGuiContext, ImGuiWindowFlags};
use imgui::{sys, Context, FontSource};
use minhook::MinHook;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::ops::{Add, DerefMut};
use std::os::raw::c_char;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use winapi::shared::minwindef::HINSTANCE;
use winapi::um::libloaderapi::GetModuleHandleW;
use crate::hook::get_dmc3_base_address;
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
const BEGIN_FUNC_ADDR: usize = 0x1F8B0;
const END_FUNC_ADDR: usize = 0x257B0;

unsafe extern "C" fn hooked_timestep() {
    if !SETUP.load(Ordering::SeqCst) {
        //set_imgui_context();
        //alt_set_context();
        MinHook::enable_hook((get_mary_base_address() + MAIN_FUNC_ADDR) as _).expect("Failed to enable hook");
        SETUP.store(true, Ordering::SeqCst);
    }
    match ORIG_TIMESTEP_FUNC.get() {
        None => {
            panic!("ORIG_TIMESTEP_FUNC not initialized in hooked render");
        }
        Some(fnc) => {
            fnc();
            log::info!("Ran orig timestep function");
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
        //log::debug!("attempt new frame");
/*        if (*sys::igGetIO()).DeltaTime < 0f32 {
            (*sys::igGetIO()).DeltaTime = 0.1;
        }*/
        // log::debug!("Frame state: {:#?}", (sys::igGetCurrentContext() as usize + 0x1b9c));
        // log::debug!("IO ptr: {:#?}", sys::igGetIO() as usize); // is in mary
        // log::debug!("Delta time {}", (*sys::igGetIO()).DeltaTime);
        // log::debug!("Frame count {}", sys::igGetFrameCount());
        //sys::igNewFrame();
        let mut flag = &mut true;
        //sys::igSetCurrentContext((get_mary_base_address() + 0x11F5D8) as *mut ImGuiContext);
        //log::debug!("mary begin: {:#}", get_mary_base_address() + BEGIN_FUNC_ADDR);
        if !read_bool_from_address(0x12c73a) {
            return;
        }
        std::mem::transmute::<_, ImGuiBegin>(get_mary_base_address() + BEGIN_FUNC_ADDR)("Archipelago".as_ptr() as *const c_char, flag as *mut bool, 0);
        //sys::igBegin("test".as_ptr() as *const c_char, flag as *mut bool, 0);
        // log::debug!("new frame");
        //text("test");
        //log::debug!("texted up");
        //sys::igRender();
        //log::debug!("rendered up");
        std::mem::transmute::<_, BasicNothingFunc>(get_mary_base_address() + END_FUNC_ADDR)();
        match ORIG_RENDER_FUNC.get() {
            None => {}
            Some(fnc) => {
                fnc();
            }
        }
    })
    /*match CTX.get_mut() {
        None => {
            log::error!("Context not available?")
        }
        Some(ctx) => {
            log::info!("About to render custom");
            HUD_INSTANCE.with(|instance| {
                log::info!("Rendering GUI");
                let mut hud = instance.borrow_mut();
                log::info!("About to frame up");
                let ui = ctx.begin();
                log::info!("New frame!");
                crate::ui::render_imgui_ui(hud.deref_mut(), ui);
            });
        }
    }*/
    // CTX.with(|ref_ctx| {
    //     log::info!("Just abouuuuut");
    //     match ref_ctx.borrow_mut().as_mut() {
    //         None => {
    //             log::error!("Context not available?")
    //         }
    //         Some(mut ctx) => {
    //             log::info!("About to render custom");
    //             HUD_INSTANCE.with(|instance| {
    //                 log::info!("Rendering GUI");
    //                 let mut hud = instance.borrow_mut();
    //                 let ui = ctx.new_frame();
    //                 log::info!("New frame!");
    //                 crate::ui::render_imgui_ui(hud.deref_mut(), ui);
    //                 ctx.render();
    //             });
    //         }
    //     }
    // });
}

pub fn text<T: AsRef<str>>(text: T) {
    let s = text.as_ref();
    unsafe {
        let start = s.as_ptr();
        let end = start.add(s.len());
        sys::igTextUnformatted(start as *const c_char, end as *const c_char);
    }
}

unsafe fn alt_set_context() {
    let orig_ptr = (get_mary_base_address() + 0x126430) as *mut ImGuiContext; // 0x11F5D8
    sys::igSetCurrentContext(orig_ptr);
}

// unsafe fn set_imgui_context() {
//     log::debug!("Setting imgui context");
//     let orig_ptr = (get_mary_base_address() + 0x11F5D8) as *mut ImGuiContext;
//     log::info!("Original context: {:?}", orig_ptr);
//     sys::igSetCurrentContext(orig_ptr);
//     let mut ctx = Context::create_internal_with_ctx(None, orig_ptr);
//     //ctx.set_context(orig_ptr);
//     log::info!("New context: {:?}", sys::igGetCurrentContext());
//     log::info!("Ctx raw: {:?}", ctx.raw);
//     ctx.io_mut().update_delta_time(Duration::from_secs(0.05 as u64));
//  /*   match File::open(&(std::env::var("windir").expect("Couldn't find windir...").add("/Fonts/consola.ttf"))) {
//         Ok(mut file) => {
//             let mut buf: Vec<u8> = Vec::new();
//             match file.read_to_end(&mut buf) {
//                 Ok(_) => {}
//                 Err(err) => {
//                     log::error!("Error reading file: {:?}", err);
//                 }
//             }
//             ctx.fonts().add_font(&[FontSource::TtfData {
//                 data: buf.as_ref(),
//                 size_pixels: 8f32,
//                 config: None,
//             }]);
//             ctx.fonts().add_font(&[FontSource::TtfData {
//                 data: buf.as_ref(),
//                 size_pixels: 16f32,
//                 config: None,
//             }]);
//             ctx.fonts().add_font(&[FontSource::TtfData {
//                 data: buf.as_ref(),
//                 size_pixels: 32f32,
//                 config: None,
//             }]);
//             ctx.fonts().add_font(&[FontSource::TtfData {
//                 data: buf.as_ref(),
//                 size_pixels: 64f32,
//                 config: None,
//             }]);
//             ctx.fonts().add_font(&[FontSource::TtfData {
//                 data: buf.as_ref(),
//                 size_pixels: 128f32,
//                 config: None,
//             }]);
//         },
//         Err(err) => {log::error!("Failed to read font: {}", err)}
//     }*/
//     CTX.replace(Some(ctx));
// }


type BasicNothingFunc = unsafe extern "system" fn(); // No args no returns

type ImGuiBegin = unsafe extern "C" fn(name: *const cty::c_char, p_open: *mut bool, flags: ImGuiWindowFlags) -> bool;

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