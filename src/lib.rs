use crate::archipelago::{
    connect_archipelago, DeathLinkData, SLOT_NUMBER, TEAM_NUMBER, TX_DEATHLINK,
};
use crate::bank::setup_bank_message_channel;
use crate::constants::Status;
use crate::hook::CLIENT;
use crate::utilities::{is_crimson_loaded, is_ddmk_loaded};
use anyhow::anyhow;
use archipelago_rs::protocol::ClientStatus;
use log::{Level, LevelFilter};
use log4rs::append::console::ConsoleAppender;
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::onstartup::OnStartUpTrigger;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::config::{Appender, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;
use std::env::current_exe;
use std::ffi::c_void;
use std::io::ErrorKind;
use std::sync::atomic::Ordering;
use std::{fs, thread};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use ui::ui::CONNECTION_STATUS;
use windows::core::{BOOL, GUID, HRESULT, PCSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::Console::{
    AllocConsole, GetConsoleMode, GetStdHandle, SetConsoleMode,
    ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
};
use windows::Win32::System::LibraryLoader::LoadLibraryA;
use xxhash_rust::const_xxh3::xxh3_64;

mod archipelago;
mod bank;
mod cache;
mod check_handler;
mod constants;
mod data;
//mod experiments;
mod config;
mod exception_handler;
mod game_manager;
mod hook;
mod item_sync;
mod location_handler;
mod mapping;
mod save_handler;
mod skill_manager;
mod ui;
mod utilities;
mod compat;

#[macro_export]
/// Does not enable the hook, that needs to be done separately
macro_rules! create_hook {
    ($offset:expr, $detour:expr, $storage:ident, $name:expr) => {{
        let target = (*DMC3_ADDRESS + $offset) as *mut _;
        let detour_ptr = ($detour as *const ()) as *mut std::ffi::c_void;
        let original = MinHook::create_hook(target, detour_ptr)?;
        $storage
            .set(std::mem::transmute(original))
            .expect(concat!($name, " hook already set"));
        //log::debug!("{name} hook created", name = $name);
    }};
}

static mut REAL_DIRECTINPUT8CREATE: Option<
    unsafe extern "system" fn(HINSTANCE, u32, GUID, *mut *mut c_void, *mut c_void) -> HRESULT,
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
                    u32,
                    GUID,
                    *mut *mut c_void,
                    *mut c_void,
                ) -> HRESULT>(b"DirectInput8Create\0")
                    .unwrap(),
            );
            std::mem::forget(lib); // Don't drop it, keep loaded
        }
    });
}

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

            thread::spawn(|| {
                load_real_dinput8();
                if current_exe().unwrap().ends_with("dmc3.exe") {
                    let mut pre_logs: Vec<PreLog> = vec![];
                    // pre_logs.push(PreLog::new(
                    //     Level::Debug,
                    //     format!("Config: {:#?}", *config::CONFIG),
                    // ));
                    match load_other_dlls(&mut pre_logs) {
                        Ok(_) => {
                            pre_logs.push(PreLog::new(
                                Level::Debug,
                                "Successfully loaded extra mods!".to_string(),
                            ));
                        }
                        Err(err) => {
                            pre_logs.push(PreLog::new(
                                Level::Error,
                                format!("Failed to load extra mods: {}", err),
                            ));
                        }
                    }
                    // Crimson will melt if the console window exists first afaik
                    create_console();
                    // I'll lose color if I do this first then console
                    setup_logger();
                    thread::spawn(|| {
                        let _ = ui::dx11_hooks::install_hook();
                    });
                    // let _ = ui::overlay::install_hook();
                    for log in pre_logs {
                        log::log!(log.level, "{}", log.message);
                    }
                    match is_file_valid("dmc3.exe", 9031715114876197692) {
                        Ok(_) => {
                            log::info!("Valid install of DMC3 detected!");
                        }
                        Err(err) => match err.kind() {
                            ErrorKind::InvalidData => {
                                log::error!(
                                    "DMC3 does not match the expected hash, bad things may occur! Please downgrade/repatch your game."
                                )
                            }
                            ErrorKind::NotFound => {
                                log::error!(
                                    "DMC3 does not exist! How in the world did you manage this"
                                );
                            }
                            _ => {
                                log::error!("Unexpected error: {}", err);
                            }
                        },
                    }
                    main_setup();
                }
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

struct PreLog {
    level: Level,
    message: String,
}

impl PreLog {
    pub fn new(level: Level, message: String) -> PreLog {
        PreLog { level, message }
    }
}

fn load_other_dlls(pre_logs: &mut Vec<PreLog>) -> Result<(), std::io::Error> {
    // The game will immolate if both of these try to load
    if !(*config::CONFIG).mods.disable_ddmk {
        match is_file_valid("Mary.dll", 7087074874482460961) {
            Ok(_) => {
                let _ = unsafe {LoadLibraryA(PCSTR::from_raw("Mary.dll\0".as_ptr()))};
                if is_ddmk_loaded() {
                    pre_logs.push(PreLog::new(
                        Level::Warn,
                        "DDMK's Actor system most likely does not work with the DeathLink setting in the randomizer, \
                please turn it off if you wish to use DeathLink".to_string()
                    ));
                }
            }
            Err(err) => match err.kind() {
                ErrorKind::InvalidData => {
                    pre_logs.push(PreLog::new(
                        Level::Error,
                        "Mary/DDMK Hash does not match version 2.7.3, please update DDMK"
                            .to_string(),
                    ));
                }
                ErrorKind::NotFound => {}
                _ => {
                    pre_logs.push(PreLog::new(
                        Level::Error,
                        format!("Unexpected error: {}", err),
                    ));
                }
            },
        }
    }
    if !(*config::CONFIG).mods.disable_crimson && !is_ddmk_loaded() {
        match is_file_valid("Crimson.dll", 6027093939875741571) {
            Ok(_) => {}
            Err(err) => match err.kind() {
                ErrorKind::InvalidData => {
                    pre_logs.push(PreLog::new(
                        Level::Error,
                        "Crimson Hash does not match version 0.4".to_string(),
                    ));
                    compat::crimson_hook::CRIMSON_HASH_ISSUE.store(true, Ordering::SeqCst);
                }
                ErrorKind::NotFound => {}
                _ => {
                    pre_logs.push(PreLog::new(
                        Level::Error,
                        format!("Unexpected error: {}", err),
                    ));
                }
            },
        }
        let _ = unsafe {LoadLibraryA(PCSTR::from_raw("Crimson.dll\0".as_ptr()))};
        if is_crimson_loaded() {
            pre_logs.push(PreLog::new(
                Level::Info,
                "Crimson has been loaded!".to_string(),
            ));
            pre_logs.push(PreLog::new(
                Level::Warn,
                "Crimson's Crimson/Style switcher mode does not work with the DeathLink setting in the randomizer, \
                please turn it off if you wish to use DeathLink".to_string()
            ));
        }
    }
    Ok(())
}

fn is_file_valid(file_path: &str, expected_hash: u64) -> Result<(), std::io::Error> {
    let data = fs::read(file_path)?;
    if xxh3_64(&data) == expected_hash {
        Ok(())
    } else {
        Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "File has invalid hash",
        ))
    }
}

fn setup_logger() {
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} {h({l})} {t} - {m}{n}")))
        .build();

    let log_file = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} {l} {t} - {m}{n}")))
        .append(false)
        .build(
            "logs/dmc3_rando_latest.log",
            Box::new(CompoundPolicy::new(
                Box::new(OnStartUpTrigger::new(10)), // 0x35c Rough guess based on the usual log output I spill out
                Box::new(
                    FixedWindowRoller::builder()
                        .build("logs/dmc3_rando_{}.log", 3)
                        .unwrap(),
                ),
            )),
        )
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("log_file", Box::new(log_file)))
        .logger(Logger::builder().build("tracing::span", LevelFilter::Warn))
        .logger(Logger::builder().build("winit::window", LevelFilter::Warn))
        .logger(Logger::builder().build("eframe::native::run", LevelFilter::Warn))
        .logger(Logger::builder().build("eframe::native::glow_integration", LevelFilter::Warn))
        .logger(Logger::builder().build("minhook", LevelFilter::Warn))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("log_file")
                .build(LevelFilter::Debug),
        )
        .unwrap();

    let _handle = log4rs::init_config(config).unwrap();
}

fn main_setup() {
    exception_handler::install_exception_handler();
    if is_ddmk_loaded() {
        log::info!("DDMK is loaded!");
        compat::ddmk_hook::setup_ddmk_hook();
    } else if is_crimson_loaded() {
        log::info!("Crimson is loaded!");
        compat::crimson_hook::setup_crimson_hook();
    } else {
        log::info!("DDMK or Crimson are not loaded!");
    }
    log::info!("DMC3 Base Address is: {:X}", *utilities::DMC3_ADDRESS);
    thread::Builder::new()
        .name("Archipelago Client".to_string())
        .spawn(move || {
            spawn_archipelago_thread();
        })
        .expect("Failed to spawn arch thread");
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn DirectInput8Create(
    hinst: HINSTANCE,
    dwVersion: u32,
    riidltf: GUID,
    ppvOut: *mut *mut c_void,
    punkOuter: *mut c_void,
) -> HRESULT {
    unsafe {
        // call into the real dinput8.dll
        load_real_dinput8(); // lazy-load if needed
        REAL_DIRECTINPUT8CREATE.expect("not loaded")(hinst, dwVersion, riidltf, ppvOut, punkOuter)
    }
}

pub fn create_console() {
    unsafe {
        if AllocConsole().is_ok() {
            pub fn enable_ansi_support() -> Result<(), anyhow::Error> {
                // So we can have sweet sweet color
                unsafe {
                    let handle = GetStdHandle(STD_OUTPUT_HANDLE)?;
                    if handle == HANDLE::default() {
                        return Err(anyhow!(windows::core::Error::from(GetLastError())));
                    }

                    let mut mode = std::mem::zeroed();
                    GetConsoleMode(handle, &mut mode)?;
                    SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING)?;
                    Ok(())
                }
            }
            match enable_ansi_support() {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Failed to enable ANSI support: {}", err);
                }
            }
            log::info!("Console created successfully!");
        } else {
            log::info!("Failed to allocate console!");
        }
    }
}

pub fn setup_deathlink_channel() -> Receiver<DeathLinkData> {
    let (tx, rx) = mpsc::channel(64);
    TX_DEATHLINK.set(tx).expect("TX already initialized");
    rx
}

#[tokio::main]
pub(crate) async fn spawn_archipelago_thread() {
    log::info!("Archipelago Thread started");
    let mut setup = false;
    let mut rx_locations = check_handler::setup_items_channel();
    let mut rx_connect = archipelago::setup_connect_channel();
    let mut rx_disconnect = archipelago::setup_disconnect_channel();
    let mut rx_bank_to_inv = setup_bank_message_channel();
    let mut rx_deathlink = setup_deathlink_channel();
    if !(*config::CONFIG).connections.disable_auto_connect {
        thread::spawn(|| {
            log::debug!("Starting auto connector");
            ui::ui::auto_connect();
        });
    }
    loop {
        // Wait for a connection request
        let Some(item) = rx_connect.recv().await else {
            log::warn!("Connect channel closed, exiting Archipelago thread.");
            break;
        };

        log::info!("Processing connection request");
        let mut client_lock = CLIENT.lock().await;

        match connect_archipelago(item).await {
            Ok(cl) => {
                client_lock.replace(cl);
                CONNECTION_STATUS.store(Status::Connected.into(), Ordering::SeqCst);
            }
            Err(err) => {
                log::error!("Failed to connect to Archipelago: {}", err);
                client_lock.take(); // Clear the client
                CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::SeqCst);
                SLOT_NUMBER.store(-1, Ordering::SeqCst);
                TEAM_NUMBER.store(-1, Ordering::SeqCst);
                continue; // Try again on next connection request
            }
        }

        // Client is successfully connected
        if let Some(ref mut client) = client_lock.as_mut() {
            if !setup {
                if let Err(err) = archipelago::run_setup(client).await {
                    log::error!("{}", err);
                }
            }
            if let Err(e) = client.status_update(ClientStatus::ClientReady).await {
                log::error!("Status update failed: {}", e);
            }
            // This blocks until a reconnect or disconnect is triggered
            archipelago::handle_things(
                client,
                &mut rx_locations,
                &mut rx_bank_to_inv,
                &mut rx_connect,
                &mut rx_deathlink,
                &mut rx_disconnect,
            )
            .await;
        }
        CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::SeqCst);
        setup = false;
        // Allow reconnection immediately without delay
    }
}
