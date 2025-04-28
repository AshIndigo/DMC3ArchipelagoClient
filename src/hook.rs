use crate::archipelago::CHECKLIST;
use crate::archipelago::{
    CONNECT_CHANNEL_SETUP, MAPPING, SLOT_NUMBER, TEAM_NUMBER, connect_archipelago,
};
use crate::bank::setup_bank_channel;
use crate::constants::{
    EventCode, ITEM_PICKED_UP_ADDR, ITEM_SPAWNS_ADDR, ItemPickedUpFunc, ItemSpawns,
    ORIGINAL_ITEM_SPAWNS, ORIGINAL_ITEMPICKEDUP, get_item,
};
use crate::experiments::asm_hook;
use crate::utilities::get_mission;
use crate::{archipelago, check_handler, constants, generated_locations, utilities};
use anyhow::{Error, anyhow};
use archipelago_rs::client::ArchipelagoClient;
use archipelago_rs::protocol::ClientStatus;
use minhook::{MH_STATUS, MinHook};
use std::arch::asm;
use std::collections::HashMap;
use std::convert::Into;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{LazyLock, Mutex, RwLockWriteGuard};
use std::{ptr, slice};
use winapi::um::libloaderapi::{FreeLibrary, GetModuleHandleW};
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::PAGE_EXECUTE_READWRITE;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    AllocConsole, ENABLE_VIRTUAL_TERMINAL_PROCESSING, FreeConsole, GetConsoleMode, GetStdHandle,
    STD_OUTPUT_HANDLE, SetConsoleMode,
};

pub fn create_console() {
    unsafe {
        if AllocConsole().is_ok() {
            pub fn enable_ansi_support() -> Result<(), Error> {
                // So we can have sweet sweet color
                unsafe {
                    let handle = GetStdHandle(STD_OUTPUT_HANDLE)?;
                    if handle == HANDLE::default() {
                        return Err(anyhow!(windows::core::Error::from_win32()));
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
#[unsafe(no_mangle)]
pub unsafe extern "system" fn free_self() -> bool {
    unsafe {
        FreeConsole().expect("Unable to free console");
        let module_handle = GetModuleHandleW(ptr::null());
        if module_handle.is_null() {
            return false;
        }
        FreeLibrary(module_handle) != 0
    }
}

pub(crate) fn install_initial_functions() {
    //asm_hook::install_jmps();
    log::info!("Installing the super trampoline for event tables");
    asm_hook::install_super_jmp_for_events(edit_event_wrapper as usize);
    setup_hooks().unwrap_or_else(|status| {
        panic!(
            "Unable to initialize hooks, randomizer is unable to function: {:?}",
            status
        )
    });
}

pub(crate) static CLIENT: LazyLock<Mutex<Option<ArchipelagoClient>>> =
    LazyLock::new(|| Mutex::new(None));

pub(crate) enum Status {
    Disconnected = 0,
    Connected = 1,
    InvalidSlot = 2,
    InvalidGame = 3,
    IncompatibleVersion = 4,
    InvalidPassword = 5,
    InvalidItemHandling = 6,
}
impl From<Status> for isize {
    fn from(value: Status) -> Self {
        match value {
            Status::Disconnected => 0,
            Status::Connected => 1,
            Status::InvalidSlot => 2,
            Status::InvalidGame => 3,
            Status::IncompatibleVersion => 4,
            Status::InvalidPassword => 5,
            Status::InvalidItemHandling => 6,
        }
    }
}

impl From<isize> for Status {
    fn from(value: isize) -> Self {
        match value {
            0 => Status::Disconnected,
            1 => Status::Connected,
            2 => Status::InvalidSlot,
            3 => Status::InvalidGame,
            4 => Status::IncompatibleVersion,
            5 => Status::InvalidPassword,
            6 => Status::InvalidItemHandling,
            _ => Status::Disconnected,
        }
    }
}

pub(crate) static CONNECTION_STATUS: AtomicIsize = AtomicIsize::new(0); // Disconnected

#[tokio::main(flavor = "current_thread")]
pub(crate) async fn spawn_arch_thread() {
    // let mut connected = false;
    let mut setup = false; // ??
    //let mut client = Err(anyhow::anyhow!("Archipelago Client not initialized"));
    log::info!("In thread");
    // For handling connection requests from the UI
    let rx_locations = check_handler::setup_items_channel();
    let rx_connect = archipelago::setup_connect_channel();
    let rx_bank = setup_bank_channel();
    CONNECT_CHANNEL_SETUP.store(true, Ordering::SeqCst); // Unneeded?

    loop {
        if CONNECT_CHANNEL_SETUP.load(Ordering::SeqCst) {
            if let Ok(rec) = rx_connect.lock() {
                while let Ok(item) = rec.try_recv() {
                    log::info!("Processing data: {}", item);
                    match connect_archipelago(item).await {
                        // worked
                        Ok(cl) => {
                            CLIENT.lock().unwrap().replace(cl);
                            CONNECTION_STATUS.store(Status::Connected.into(), Ordering::SeqCst);
                        }
                        Err(err) => {
                            log::error!("Failed to connect to archipelago: {}", err);
                            CLIENT.lock().unwrap().take(); // Clear out the CLIENT field
                            CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::SeqCst);
                            SLOT_NUMBER.store(-1, Ordering::SeqCst);
                            TEAM_NUMBER.store(-1, Ordering::SeqCst);
                        }
                    }
                }
            }
        }
        match CLIENT.lock().unwrap().as_mut() {
            // If the client exists and is usable, then try to initially set things up and then handle messages
            Some(ref mut cl) => {
                if setup == false {
                    cl.status_update(ClientStatus::ClientConnected)
                        .await
                        .expect("Status update failed?");
                    archipelago::run_setup(cl).await;
                    log::info!("Synchronizing items");
                    archipelago::sync_items(cl).await;
                    setup = true;
                }
                archipelago::handle_things(cl, &rx_locations, &rx_bank).await;
            }
            None => {
                CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::SeqCst);
            }
        }
    }
}

/// Set the starting gun and melee weapon upon a new game
pub unsafe fn set_starting_weapons(melee_id: u8, gun_id: u8) {
    unsafe {
        utilities::replace_single_byte(constants::STARTING_MELEE, melee_id); // Melee weapon
        utilities::replace_single_byte(constants::STARTING_GUN, gun_id); // Gun
        todo!()
    }
}

pub unsafe fn edit_event_wrapper() {
    unsafe {
        pub unsafe fn edit_event_drop() {
            unsafe {
                // Basic values
                let Some(mapping) = MAPPING.get() else { return };
                // Get events for the specific mission
                let Some(mission_event_tables) = constants::EVENT_TABLES.get(&get_mission()) else {
                    return;
                };
                let Ok(checked_locations) = archipelago::get_checked_locations().lock() else {
                    return;
                };

                // For each table
                for event_table in mission_event_tables {
                    for event in &event_table.events {
                        if checked_locations.contains(&event_table.location) {
                            log::debug!("Event loc checked: {}", &event_table.location);
                            // If the location has been checked, disable it by making the game check for an item the player already has
                            if event.event_type == EventCode::CHECK {
                                utilities::replace_single_byte_no_offset(
                                    constants::EVENT_TABLE_ADDR + event.offset,
                                    0x00,
                                ) // Use 0x00, trying to use rebellion doesn't work
                            }
                            // if (event.event_type == EventCode::GIVE) {
                            //     replace_single_byte_no_offset(base_table + event.offset, tables::get_item_id(mapping.items.get(&tbl.location).unwrap()).unwrap())
                            // }
                            if event.event_type == EventCode::END {
                                utilities::replace_single_byte_no_offset(
                                    constants::EVENT_TABLE_ADDR + event.offset,
                                    0x00,
                                )
                            }
                        } else {
                            // Location has not been checked off! TODO Make the "check" event, a dummied item
                            log::debug!("Event loc not checked: {}", &event_table.location);
                            utilities::replace_single_byte_no_offset(
                                constants::EVENT_TABLE_ADDR + event.offset,
                                constants::get_item_id(
                                    mapping.items.get(&event_table.location).unwrap(),
                                )
                                .unwrap(),
                            )
                        }
                    }
                }
            }
        }

        asm!(
            "sub rsp, 72",
            "push rcx",
            "push rdx",
            "push rbx",
            "push r11",
            "push r10",
            "push r9",
            "push r8",
            "push rsi",
            "call {}", // Call the function
            "pop rsi",
            "pop r8",
            "pop r9",
            "pop r10",
            "pop r11",
            "pop rbx",
            "pop rdx",
            "pop rcx",
            "add rsp, 72",
            sym edit_event_drop,
            //clobber_abi("win64"),
        );
    }
}

/// Modify the game's code so the "pickup mode" table is correct
// start at 1B3944 -> 1B395A
// Set these from 01 to 02
pub(crate) unsafe fn rewrite_mode_table() {
    unsafe {
        let table_address = constants::ITEM_MODE_TABLE + utilities::get_dmc3_base_address();
        let mut old_protect = 0;
        let length = 16;
        VirtualProtect(
            table_address as *mut _,
            length, // Length of table I need to modify
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        let table = slice::from_raw_parts_mut(table_address as *mut u8, length);
        table.fill(0x01u8); // 0 = orbs, 1 = items, 2 = bad

        VirtualProtect(
            table_address as *mut _,
            length,
            old_protect,
            &mut old_protect,
        );
    }
}

/// Modifies Adjudicator Drops
pub(crate) unsafe fn modify_adjudicator_drop() {
    unsafe {
        match MAPPING.get() {
            Some(mapping) => {
                for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
                    // Run through all locations
                    if entry.adjudicator && entry.mission == get_mission() as u8 {
                        // If Location is adjudicator and mission numbers match
                        let item_id =
                            constants::get_item_id(mapping.items.get(*location_name).unwrap())
                                .unwrap(); // Get the item ID and replace
                        utilities::replace_single_byte(constants::ADJUDICATOR_ITEM_ID_1, item_id);
                        utilities::replace_single_byte(constants::ADJUDICATOR_ITEM_ID_2, item_id);
                    }
                }
            }
            _ => {}
        }
    }
}

unsafe fn item_spawns_hook(unknown: i64) {
    unsafe {
        #[allow(unused_assignments)]
        let mut item_addr: *mut i32 = ptr::null_mut();
        let item_count: u32;
        asm!(
            "mov eax, [rcx+0x06]",
            out("eax") item_count, // Count of items in room
            clobber_abi("win64")
        );
        asm!(
            "lea eax, [rcx+0x10]",
            out("eax") item_addr, // Item address, needs to be [eax]'d
            clobber_abi("win64")
        );
        log::debug!("Item count: {:x}", item_count);
        let room_num: u16 = utilities::get_room() as u16;
        set_relevant_key_items();
        match MAPPING.get() {
            Some(mapping) => {
                modify_adjudicator_drop();
                for _i in 0..item_count {
                    let item_ref: &u32 = &*(item_addr as *const u32);
                    log::debug!(
                        "Item ID: {} (0x{:x})",
                        get_item(*item_ref as u64),
                        *item_ref
                    );
                    for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
                        if entry.room_number == 0 {
                            // Skipping if location file has room as 0, that means its either event or not done
                            continue;
                        }
                        //log::debug!("Room number X: {} Room number memory: {}, Item ID X: 0x{:x}, Item ID Memory: 0x{:x}", entry.room_number, room_num, entry.item_id, *item_ref);
                        if entry.room_number == room_num
                            && entry.item_id as u32 == *item_ref
                            && !entry.adjudicator
                        {
                            let ins_val =
                                constants::get_item_id(mapping.items.get(*location_name).unwrap()); // Scary
                            *item_addr = ins_val.unwrap() as i32;
                            log::info!(
                                "Replaced item in room {} ({}) with {} 0x{:x}",
                                entry.room_number,
                                location_name,
                                get_item(*item_ref as u64),
                                ins_val.unwrap() as i32
                            );
                        }
                    }
                    item_addr = item_addr.byte_offset(0x14);
                }
            }
            None => {
                log::error!("Mapping's are not set up");
            }
        }
        if let Some(original) = ORIGINAL_ITEM_SPAWNS {
            original(unknown);
        }
    }
}

fn set_relevant_key_items() {
    let checklist: RwLockWriteGuard<HashMap<String, bool>> =
        CHECKLIST.get().unwrap().write().unwrap();
    let current_inv_addr = utilities::read_usize_from_address(constants::INVENTORY_PTR);
    log::debug!("Current INV Addr: 0x{:x}", current_inv_addr);
    log::debug!("Resetting high roller card");
    /* let item_addr = current_inv_addr + 0x60 + tables::KEY_ITEMS.iter().position(|&str| str == *item).unwrap();
    log::debug!("Attempting to replace at address: 0x{:x} with flag 0x{:x}", item_addr, flag);
    unsafe { utilities::replace_single_byte_no_offset(item_addr, flag) };*/
    let mut flag: u8;
    match constants::MISSION_ITEM_MAP.get(&get_mission()) {
        None => {} // No items for the mission
        Some(item_list) => {
            for item in item_list.into_iter() {
                if *checklist.get(*item).unwrap_or(&false) {
                    flag = 0x01;
                    log::debug!("Item Relevant to mission {}", *item)
                } else {
                    flag = 0x00;
                }
                let item_addr = current_inv_addr
                    + 0x60
                    + constants::KEY_ITEM_OFFSETS.get(item).unwrap().clone() as usize;
                log::debug!(
                    "Attempting to replace at address: 0x{:x} with flag 0x{:x}",
                    item_addr,
                    flag
                );
                unsafe { utilities::replace_single_byte_no_offset(item_addr, flag) };
            }
        }
    }
}

// 23d680 - Pause menu event? Hook in here to do rendering
fn setup_hooks() -> Result<(), MH_STATUS> {
    unsafe {
        let original_picked_up = utilities::get_dmc3_base_address() + ITEM_PICKED_UP_ADDR;
        ORIGINAL_ITEMPICKEDUP = Some(std::mem::transmute::<_, ItemPickedUpFunc>(
            MinHook::create_hook(
                original_picked_up as _,
                check_handler::item_picked_up_hook as _,
            )?,
        ));
        MinHook::enable_hook(original_picked_up as _)?;
        log::info!("Item picked up hook enabled");
        let original_spawn = utilities::get_dmc3_base_address() + ITEM_SPAWNS_ADDR;
        ORIGINAL_ITEM_SPAWNS = Some(std::mem::transmute::<_, ItemSpawns>(MinHook::create_hook(
            original_spawn as _,
            item_spawns_hook as _,
        )?));
        MinHook::enable_hook(original_spawn as _)?;
        log::info!("Item spawn hook enabled");
        Ok(())
    }
}

/// The mapping data at dmc3.exe+5c4c20+1A00
pub unsafe fn modify_itm_table(offset: usize, id: u8) {
    unsafe {
        // let start_addr = 0x5C4C20usize; dmc3.exe+5c4c20+1A00
        // let end_addr = 0x5C4C20 + 0xC8; // 0x5C4CE8
        let true_offset = offset + utilities::get_dmc3_base_address() + 0x1A00usize; // MFW I can't do my offsets correctly
        if offset == 0x0 {
            return; // Undecided/ignorable
        }
        let mut old_protect = 0;
        VirtualProtect(
            true_offset as *mut _,
            4, // Length of table I need to modify
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        let table = slice::from_raw_parts_mut(true_offset as *mut u8, 4);

        table[3] = id;

        VirtualProtect(true_offset as *mut _, 4, old_protect, &mut old_protect);
        log::debug!(
            "Modified Item Table: Address: 0x{:x}, ID: 0x{:x}, Offset: 0x{:x}",
            true_offset,
            id,
            offset
        );
    }
}

pub(crate) fn can_add_item(item_name: &&str) -> bool {
    todo!();
}

pub(crate) fn add_item(item_name: &String) {
    todo!()
}
