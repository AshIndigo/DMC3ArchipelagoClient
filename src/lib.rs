use crate::archipelago::{
    connect_archipelago, DeathLinkData, TX_DEATHLINK,
};
use crate::bank::setup_bank_message_channel;
use crate::utilities::{is_crimson_loaded, is_ddmk_loaded};
use archipelago_rs::protocol::ClientStatus;
use std::sync::atomic::Ordering;
use std::thread;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use connection_manager::CONNECTION_STATUS;
use windows::core::BOOL;
use windows::Win32::Foundation::*;
use randomizer_utilities::archipelago_utilities::{CLIENT, SLOT_NUMBER, TEAM_NUMBER};
use randomizer_utilities::exception_handler;
use randomizer_utilities::ui_utilities::Status;

mod archipelago;
mod bank;
mod check_handler;
mod constants;
mod data;
//mod experiments;
mod compat;
mod config;
mod game_manager;
mod hook;
mod location_handler;
mod mapping;
mod save_handler;
mod skill_manager;
mod ui;
mod utilities;
pub(crate) mod connection_manager;

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

#[derive(Debug)]
#[repr(C)]
pub(crate) struct LoaderStatus {
    pub crimson_hash_error: bool,
    pub dmc3_hash_error: bool,
}

/*#[link(name = "dinput8")]
unsafe extern "C" {
    pub(crate) fn get_loader_status() -> &'static LoaderStatus;
}
*/
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
            randomizer_utilities::setup_logger("dmc3_rando");
            //let loader_status = unsafe { get_loader_status() };
            //log::debug!("loader_status: {loader_status:?}");
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
    log::info!("DMC3 Base Address is: {:X}", *utilities::DMC3_ADDRESS);
    thread::Builder::new()
        .name("Archipelago Client".to_string())
        .spawn(move || {
            spawn_archipelago_thread();
        })
        .expect("Failed to spawn arch thread");
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
    if !config::CONFIG.connections.disable_auto_connect {
        thread::spawn(|| {
            log::debug!("Starting auto connector");
            connection_manager::auto_connect();
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
                log::error!("Failed to connect to Archipelago: {err}");
                client_lock.take(); // Clear the client
                CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::SeqCst);
                SLOT_NUMBER.store(-1, Ordering::SeqCst);
                TEAM_NUMBER.store(-1, Ordering::SeqCst);
                continue; // Try again on next connection request
            }
        }

        // Client is successfully connected
        if let Some(ref mut client) = client_lock.as_mut() {
            if !setup && let Err(err) = archipelago::run_setup(client).await {
                log::error!("{err}");
            }

            if let Err(e) = client.status_update(ClientStatus::ClientReady).await {
                log::error!("Status update failed: {e}");
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
