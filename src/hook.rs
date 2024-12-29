use crate::archipelago::{Mapping, CHECKED_LOCATIONS, MAPPING};
use crate::{archipelago, constants, tables};
use archipelago_rs::protocol::ClientStatus;
use once_cell::sync::OnceCell;
use std::arch::asm;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::os::windows::ffi::OsStrExt;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::{ptr, slice};
use std::cmp::PartialEq;
use std::collections::HashMap;
use archipelago_rs::client::ArchipelagoClient;
use log::{LevelFilter, SetLoggerError};
use simple_logger::SimpleLogger;
use winapi::shared::minwindef::{HINSTANCE, LPVOID};
use winapi::um::libloaderapi::{FreeLibrary, GetModuleHandleW};
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::PAGE_EXECUTE_READWRITE;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::Console::{AllocConsole, FreeConsole};
use crate::tables::{set_event_tables, EventCode, EventTable};
use crate::ui::start_ui;

const TARGET_FUNCTION: usize = 0x1b4595;

static TX: OnceCell<Sender<Location>> = OnceCell::new();

pub(crate) struct Location {
    pub(crate) item_id: u64,
    pub(crate) room: i32,
    pub(crate) mission: i32
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(
            write!(f, "Room ID: {:#} Item ID: {:#x}", self.room, self.item_id)
                .expect("Failed to print Location as String!"),
        )
    }
}

#[no_mangle]
pub unsafe extern "system" fn check_off_location() {
    //noinspection RsBorrowChecker // To make RustRover quiet down
    // This does not work for event weapons...
    unsafe extern "system" fn send_off() {
        let item_id: u64;
        asm!(
            "movzx r9d, byte ptr [rcx+0x60]", // To capture item_id
            out("r9d") item_id,
            clobber_abi("win64")
        );
        if let Some(tx) = TX.get() {
            tx.send(Location {
                item_id, // This is fine
                room: get_room(),
                mission: get_mission()
            })
            .expect("Failed to send Location!");
        }
    }

    asm!(
        "sub rsp, 8",
        "push rcx",
        "push rdx",
        "push rbx",
        "push r11",
        "push r10",
        "push r9",
        "push r8",
        "call {}",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop rbx",
        "pop rdx",
        "pop rcx",
        "add rsp, 8",
        "movzx r9d, byte ptr [rcx+0x60]", // Original code
        sym send_off,
        clobber_abi("win64"),
        out("rax") _,
        out("rsi") _,
        out("rdi") _,
        out("r12") _,
        out("r13") _,
        out("r14") _,
        out("r15") _,
    );
}

fn read_int_from_address(address: usize) -> i32 {
    unsafe { *((address + get_dmc3_base_address()) as *const i32) }
}

#[no_mangle]
pub unsafe extern "system" fn get_dmc3_base_address() -> usize {
    let wide_name: Vec<u16> = OsStr::new("dmc3.exe")
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

fn install_super_jmp_for_events(custom_function: usize) {
    // This is for Location checking
    unsafe {
        modify_call_offset(0x1af0f8usize + get_dmc3_base_address(), 6); //sub
        let first_target_address = get_dmc3_base_address() + 0x1A9BBAusize; // This is for the 6 bytes above 1a9bc0
        let mut old_protect = 0;
        let length_first = 6;
        VirtualProtect(
            first_target_address as *mut _,
            length_first, // MOV + JMP = 12 bytes
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        // Write the absolute jump (MOV RAX + JMP RAX)
        let target_code = slice::from_raw_parts_mut(first_target_address as *mut u8, length_first);
        target_code[0] = 0xE8;
        target_code[1] = 0x82;
        target_code[2] = 0xFE;
        target_code[3] = 0xFF;
        target_code[4] = 0xFF;

        // Restore the original memory protection
        VirtualProtect(first_target_address as *mut _, length_first, old_protect, &mut old_protect);

        log::debug!(
            "Installed 1st trampoline: Target Address = 0x{:x}, Custom Function = 0x{:x}",
            first_target_address, custom_function
        );

        let second_target_address = get_dmc3_base_address() + 0x1A9A41usize;
        let mut old_protect = 0;
        let length_second = 16;
        VirtualProtect(
            second_target_address as *mut _,
            length_second, // MOV + JMP = 12 bytes
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        // Write the absolute jump (MOV RAX + JMP RAX)
        let target_code_second = slice::from_raw_parts_mut(second_target_address as *mut u8, length_second);
        target_code_second[0] = 0x50; // Push RAX
        target_code_second[1] = 0x48; // REX.W
        target_code_second[2] = 0xB8; // MOV RAX, imm64
        target_code_second[3..11].copy_from_slice(&custom_function.to_le_bytes());

        target_code_second[11] = 0xFF; // JMP opcode
        target_code_second[12] = 0xD0; // JMP RAX
        target_code_second[13] = 0x58; // POP Rax
        target_code_second[14] = 0xC3; // RET
        // Restore the original memory protection
        VirtualProtect(second_target_address as *mut _, length_second, old_protect, &mut old_protect);
    }
}

fn install_jump_rax_for_itm_file(custom_function: usize) {
    // This is for Location checking
    unsafe {
        modify_call_offset(0x23ba41usize + get_dmc3_base_address(), 13); //sub
        modify_call_offset(0x23ce70usize + get_dmc3_base_address(), 13); //sub fixes key items as well...
        let target_address = get_dmc3_base_address() + 0x1B4433usize;
        // Step 1: Modify memory protection to allow writing
        let mut old_protect = 0;
        VirtualProtect(
            target_address as *mut _,
            13, // MOV + JMP = 12 bytes
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        // Write the absolute jump (MOV RAX + JMP RAX)
        let target_code = slice::from_raw_parts_mut(target_address as *mut u8, 16);

        // MOV RAX, custom_function
        target_code[0] = 0x50; // Push RAX
        target_code[1] = 0x48; // REX.W
        target_code[2] = 0xB8; // MOV RAX, imm64
        target_code[3..11].copy_from_slice(&custom_function.to_le_bytes()); // TODO Could I replace this asm! and sym?

        // JMP (Call) RAX
        target_code[11] = 0xFF; // JMP opcode
        target_code[12] = 0xD0; // JMP RAX
        target_code[13] = 0x58; // POP Rax
                                // for i in 14..13 {
                                //     target_code[i] = 0x90; // NOP
                                // }

        // Restore the original memory protection
        VirtualProtect(target_address as *mut _, 13, old_protect, &mut old_protect);

        log::debug!(
            "Installed absolute jump: Target Address = 0x{:x}, Custom Function = 0x{:x}",
            target_address, custom_function
        );
    }
}

/// This is for in world pickups only, i.e orbs, key items (Astro board), items on the ground (M2 Vital Star)
fn install_jump_rax_for_in_world(custom_function: usize) {
    // This is for Location checking
    unsafe {
        modify_call_offset(0x1b7143usize + get_dmc3_base_address(), 11); //sub
        modify_jmp_offset(0x1b5ADDusize + get_dmc3_base_address(), 11); //sub fixes key items as well...
        let target_address = get_dmc3_base_address() + TARGET_FUNCTION;
        // Modify memory protection to allow writing
        let mut old_protect = 0;
        VirtualProtect(
            target_address as *mut _,
            16, // MOV + JMP = 12 bytes
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        // Write the absolute jump (MOV RAX + JMP RAX)
        let target_code = slice::from_raw_parts_mut(target_address as *mut u8, 16);

        // MOV RAX, custom_function
        target_code[0] = 0x50; // Push RAX
        target_code[1] = 0x48; // REX.W
        target_code[2] = 0xB8; // MOV RAX, imm64
        target_code[3..11].copy_from_slice(&custom_function.to_le_bytes());

        // JMP (Call) RAX
        target_code[11] = 0xFF; // JMP opcode
        target_code[12] = 0xD0; // JMP RAX
        target_code[13] = 0x58; // POP Rax
        for i in 14..16 {
            target_code[i] = 0x90; // NOP
        }

        // Restore the original memory protection
        VirtualProtect(target_address as *mut _, 16, old_protect, &mut old_protect);

        log::debug!(
            "Installed absolute jump: Target Address = 0x{:x}, Custom Function = 0x{:x}",
            target_address, custom_function
        );
    }
}

/// Modifies a CALL instructions offset, subtracting it by the given value
fn modify_call_offset(call_address: usize, modify: i32) {
    unsafe {
        // Modify memory protection to allow writing
        let mut old_protect = 0;
        VirtualProtect(
            call_address as *mut _,
            5, // CALL opcode + 4-byte offset = 5 bytes
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        // Read the existing offset
        let call_code = slice::from_raw_parts_mut(call_address as *mut u8, 5);

        // Ensure the opcode is CALL (0xE8)
        if call_code[0] != 0xE8 {
            panic!(
                "Instruction at 0x{:x} is not a CALL instruction. Opcode: 0x{:x}",
                call_address, call_code[0]
            );
        }

        // Extract the existing 4-byte relative offset
        let existing_offset = i32::from_le_bytes(call_code[1..5].try_into().unwrap());

        // Calculate the new offset
        let new_offset = existing_offset.wrapping_sub(modify);

        call_code[1..5].copy_from_slice(&new_offset.to_le_bytes());

        // Restore the original memory protection
        VirtualProtect(call_address as *mut _, 5, old_protect, &mut old_protect);

        log::debug!(
            "Modified CALL instruction at 0x{:x}: Old Offset = 0x{:x}, Modify = {}, New Offset = 0x{:x}",
            call_address, existing_offset, modify, new_offset
        );
    }
}

/// Modifies a JMP instructions offset, subtracting it by the given value
fn modify_jmp_offset(call_address: usize, modify: i32) {
    unsafe {
        // Step 1: Modify memory protection to allow writing
        let mut old_protect = 0;
        VirtualProtect(
            call_address as *mut _,
            5, // CALL opcode + 4-byte offset = 5 bytes
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        // Step 2: Read the existing offset
        let call_code = slice::from_raw_parts_mut(call_address as *mut u8, 5);

        // Check if is JMP, otherwise panic
        if call_code[0] != 0xE9 {
            panic!(
                "Instruction at 0x{:x} is not a JMP instruction. Opcode: 0x{:x}",
                call_address, call_code[0]
            );
        }

        // Extract the existing 4-byte relative offset
        let existing_offset = i32::from_le_bytes(call_code[1..5].try_into().unwrap());

        // Step 3: Calculate the new offset
        let new_offset = existing_offset.wrapping_sub(modify);

        call_code[1..5].copy_from_slice(&new_offset.to_le_bytes());

        // Step 5: Restore the original memory protection
        VirtualProtect(call_address as *mut _, 5, old_protect, &mut old_protect);

        log::info!(
            "Modified JMP instruction at 0x{:x}: Old Offset = 0x{:x}, Modify = {}, New Offset = 0x{:x}",
            call_address, existing_offset, modify, new_offset
        );
    }
}

#[no_mangle]
pub unsafe extern "system" fn create_console() {
    unsafe {
        if AllocConsole().is_ok() {
            log::info!("Console created successfully!");
        } else {
            log::info!("Failed to allocate console!");
        }
    }
}

#[no_mangle]
pub unsafe extern "system" fn free_self() -> bool {
    unsafe {
        FreeConsole().expect("Bai bai console");
        let module_handle = GetModuleHandleW(ptr::null());
        if module_handle.is_null() {
            return false;
        }
        FreeLibrary(module_handle) != 0
    }
}

fn setup_channel() -> Arc<Mutex<Receiver<Location>>> {
    let (tx, rx) = mpsc::channel();
    TX.set(tx).expect("TX already initialized");
    Arc::new(Mutex::new(rx))
}

#[no_mangle]
pub extern "system" fn DllMain(
    _hinst_dll: HINSTANCE,
    fdw_reason: u32,
    _lpv_reserved: LPVOID,
) -> BOOL {
    const DLL_PROCESS_ATTACH: u32 = 1;
    const DLL_PROCESS_DETACH: u32 = 0;
    const DLL_THREAD_ATTACH: u32 = 2;
    const DLL_THREAD_DETACH: u32 = 3;
    
    const USE_GUI: bool = false;


    match fdw_reason {
        DLL_PROCESS_ATTACH => unsafe {
            if USE_GUI {
                match egui_logger::builder().init() {
                    Ok(_) => {log::info!("Logger initialized")}
                    Err(err) => {log::error!("Failed to setup logger: {}", err);}
                }
            } else {
                SimpleLogger::new().with_module_level("tokio", LevelFilter::Warn).with_module_level("tungstenite::protocol", LevelFilter::Warn).with_threads(true).init().unwrap();
                create_console();
            }
            let rx = setup_channel();
            thread::Builder::new()
                .name("Archipelago Client".to_string())
                .spawn(move || {
                    if USE_GUI {
                        start_ui();
                    }
                    spawn_arch_thread(rx);
                })
                .expect("Failed to spawn arch thread");
            log::info!("Installing trampoline for in-world items");
            install_jump_rax_for_in_world(check_off_location as usize);
            log::info!("Installing trampoline for modifying itm files");
            install_jump_rax_for_itm_file(modify_itm as usize);
            log::info!("Installing the super trampoline for event tables");
            install_super_jmp_for_events(edit_event_wrapper as usize);
            match tables::EVENT_TABLES.set(set_event_tables()) {
                Ok(_) => {log::info!("EVENT_TABLES was set successfully")}
                Err(_) => {}
            }
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
#[tokio::main(flavor = "current_thread")]
async unsafe fn spawn_arch_thread(rx: Arc<Mutex<Receiver<Location>>>) {
    let mut connected = false;
    let mut setup = false;
    let mut client = Err(anyhow::anyhow!("Archipelago Client not initialized"));
    log::info!("In thread");
    loop {
        if connected == false {
            log::info!("Going for connection");
            client = archipelago::connect_archipelago().await;
            match &client {
                Ok(cl) => {
                    log::debug!("Room Info: {:?}", cl.room_info());
                    connected = true
                }
                Err(..) => log::warn!("Failed to connect"),
            }
        }
        match client {
            Ok(ref mut cl) => {
                if setup == false {
                    cl.status_update(ClientStatus::ClientConnected)
                        .await
                        .expect("Status update failed?");
                    archipelago::run_setup(cl).await;
                    setup = true;
                }
                archipelago::handle_things(cl, &rx).await;
            }
            Err(..) => log::info!("Not connected?"),
        }
    }
}

/// Set the starting gun and melee weapon upon a new game
pub unsafe fn set_starting_weapons(melee_id: u8, gun_id: u8) {
    replace_single_byte(0x000, melee_id); // Melee weapon
    replace_single_byte(0x000, gun_id); // Gun

}

pub unsafe fn edit_event_wrapper() {
    pub unsafe fn edit_event_drop() {
        match MAPPING.get() {
            None => {}
            Some(mapping) => {
                let mission_num = get_mission();
                let base_table = 0x01A42680usize; // TODO is this gonna be ok?
                match tables::EVENT_TABLES.get() {
                    Some(tables) => {
                        let act_tables = tables;
                        match act_tables.get(&mission_num) {
                            None => {}
                            Some(event_tables) => {
                                for tbl in event_tables {
                                    for event in &tbl.events {
                                        match CHECKED_LOCATIONS.get() {
                                            None => {}
                                            Some(chk_locs) => {
                                                if chk_locs.contains(&tbl.location) { // If the location has been checked, disable it by making the game check for an itme the player already has
                                                    if event.event_type == EventCode::CHECK {
                                                        replace_single_byte_no_offset(base_table + event.offset, 0x16) // TODO Default starting weapon
                                                    }
                                                    // if (event.event_type == EventCode::GIVE) {
                                                    //     replace_single_byte_no_offset(base_table + event.offset, tables::get_item_id(mapping.items.get(&tbl.location).unwrap()).unwrap())
                                                    // }
                                                } else { // Location has not been checked off!
                                                    replace_single_byte_no_offset(base_table + event.offset, tables::get_item_id(mapping.items.get(&tbl.location).unwrap()).unwrap())
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                    },
                    None => {

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

/// Modify the game's code so the "pickup mode" table is correct
// start at 1B3944 -> 1B395A
// Set these from 01 to 02
pub(crate) unsafe fn rewrite_mode_table() {
    let table_address = 0x1B4534usize + get_dmc3_base_address();
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

    VirtualProtect(table_address as *mut _, length, old_protect, &mut old_protect);
}

unsafe fn replace_single_byte(offset: usize, new_val: u8) {
    let length = 1;
    let offset = offset + get_dmc3_base_address();
    let mut old_protect = 0;
    VirtualProtect(
        offset as *mut _,
        length,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    );

    let table = slice::from_raw_parts_mut(offset as *mut u8, length);
    table[0] = new_val;

    VirtualProtect(offset as *mut _, length, old_protect, &mut old_protect);
    log::debug!("Modified byte at: Offset: {:x}, byte: {:x}", offset, new_val);
}

unsafe fn replace_single_byte_no_offset(offset: usize, new_val: u8) {
    let length = 1;
    let offset = offset;
    let mut old_protect = 0;
    VirtualProtect(
        offset as *mut _,
        length,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    );

    let table = slice::from_raw_parts_mut(offset as *mut u8, length);
    table[0] = new_val;

    VirtualProtect(offset as *mut _, length, old_protect, &mut old_protect);
    log::debug!("Modified byte at: Offset: {:x}, byte: {:x}", offset, new_val);
}

unsafe fn modify_adjudicator_drop() {
    let mission_number = get_mission();
    match MAPPING.get() {
        Some(mapping) => {
            for (location_name, entry) in constants::get_locations() {
                if entry.adjudicator && entry.mission == mission_number as u8 {
                    let item_id = tables::get_item_id(mapping.items.get(location_name).unwrap()).unwrap();
                    replace_single_byte(0x250594, item_id);
                    replace_single_byte(0x25040d, item_id);
                }
            }
        }
        _ => {}
    }
}

/// Get current mission
fn get_mission() -> i32 {
    read_int_from_address(0xC8F250usize)
}

/// Get current room
fn get_room() -> i32 {
    read_int_from_address(0xC8F258usize)
}

unsafe fn modify_itm() {
    //noinspection RsBorrowChecker
    unsafe fn modify_itm_memory() {
        let itm_addr: *mut i32;
        let item_id : u32;
        asm!(
            "lea edx, [rcx+0x10]",
            "mov eax, [edx]",
            out("edx") itm_addr,
            out("eax") item_id, // TODO would be cool to reduce this even more
            // TODO This doesn't work for ITM files that have multiple items and the one we want to change is not the 1st item (ex. Room 5)
            clobber_abi("win64")
        );
        modify_adjudicator_drop(); // Should be fine right here?
        match MAPPING.get() {
            Some(mapping) => {
                let room_num: u16 = get_room() as u16; // read_int_from_address(0xC8F258usize) as u16;
                for (location_name, entry) in constants::get_locations() {
                    if entry.room_number == 0 { // Skipping if location file has room as 0, that means its either event or not done
                        continue;
                    }
                    //log::debug!("Room number X: {} Room number memory: {}, Item ID X: 0x{:x}, Item ID Memory: 0x{:x}", entry.room_number, room_num, entry.item_id, item_id);
                    if entry.room_number == room_num && entry.item_id as u32 == item_id {
                        let ins_val = tables::get_item_id(mapping.items.get(location_name).unwrap()); // Scary
                        *itm_addr = ins_val.unwrap() as i32;
                        log::info!("Replaced item in room {} ({}) with 0x{:x}", entry.room_number, location_name, ins_val.unwrap() as i32);
                    }
                }
            }
            None => {
                log::warn!("No mappings found!");
            }
        }
    }

    asm!(
        "sub rsp, 16",
        "push rcx",
        "push rdx",
        "push rbx",
        "push r11",
        "push r10",
        "push r9",
        "push r8",
        "push rsi", // Preserve rsi
        "call {}", // Call the function
        "pop rsi", // Restore rsi
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop rbx",
        "pop rdx",
        "pop rcx",
        "add rsp, 16",
        sym modify_itm_memory,
        clobber_abi("win64"),
    );
}

pub unsafe fn modify_itm_table(offset: usize, id: u8) {
    // let start_addr = 0x5C4C20usize; dmc3.exe+5c4c20+1A00
    // let end_addr = 0x5C4C20 + 0xC8; // 0x5C4CE8
    let true_offset = offset + get_dmc3_base_address() + 0x1A00usize; // MFW I can't do my offsets correctly
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
    //log::debug!("Modified item table: Offset: {:x}, id: {}", true_offset, id);
}
