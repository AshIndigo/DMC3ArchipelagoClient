use anyhow::anyhow;
use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use archipelago_rs::protocol::{ClientStatus, GameData, RoomInfo};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::arch::asm;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::fs::{remove_file, File};
use std::io::{BufRead, BufReader, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::{io, thread};
use std::{ptr, slice};
use winapi::shared::minwindef::{HINSTANCE, LPVOID};
use winapi::um::libloaderapi::{FreeLibrary, GetModuleHandleW};
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::PAGE_EXECUTE_READWRITE;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::Console::{AllocConsole, FreeConsole};

const TARGET_FUNCTION: usize = 0x1b4595;

static TX: OnceCell<Sender<Location>> = OnceCell::new();

struct Location {
    item_id: u64,
    room: i32,
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
pub unsafe extern "system" fn check_off_location() { //noinspection RsBorrowChecker // To make RustRover quiet down
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
                room: read_int_from_address(0xC8F258),
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


#[allow(dead_code)]
fn get_item(item_id: u64) -> &'static str {
    match item_id {
        0x00 => "Red Orb - 1",
        0x01 => "Red Orb - 5",
        0x02 => "Red Orb - 20",
        0x03 => "Red Orb - 100",
        0x04 => "Red Orb - 1000",
        0x05 => "Gold Orb",
        0x06 => "Yellow Orb",
        0x07 => "Blue Orb (No Work)",
        0x08 => "Purple Orb (No Work)",
        0x09 => "Blue Orb Frag",
        0x0A => "Green Orb",
        0x0B => "Grorb",
        0x0C => "Big Green Orb",
        0x0D => "TODO",
        0x0E => "TODO",
        0x0F => "TODO",
        0x10 => "Vital Star L",
        0x11 => "Vital Star S",
        0x12 => "Devil Star",
        0x13 => "Holy Water",
        0x14 => "Reb Orb (Fear Test Test)",
        0x15 => "Amulet (Casino Coins)",
        0x16 => "Rebellion (Normal)",
        0x17 => "Cerberus",
        0x18 => "Agni?",
        0x19 => "Rebellion Awakened",
        0x1A => "Nevan",
        0x1B => "Beowulf",
        0x1C => "E&I",
        0x1D => "Shotgun",
        0x1E => "Artemis(?)",
        0x1F => "Spiral(?)",
        0x20 => "Red Orb...? (Bomb!)",
        0x21 => "Kalina Ann",
        0x22 => "Quicksilver",
        0x23 => "Dopl Style",
        0x24 => "Astro Board",
        0x25 => "Vajura",
        0x26 => "High Roller Card",
        0x27 => "Soul of Steel",
        0x28 => "Essence of Fighting",
        0x29 => "Essence of Technique",
        0x2A => "Essence of Intelligence",
        0x2B => "Orichalcum Frag",
        0x2C => "TODO",
        0x2D => "TODO",
        0x2E => "TODO",
        0x2F => "TODO",
        0x30 => "Stone Mask",
        0x31 => "Neo Gen",
        0x32 => "Haywire Neo",
        0x33 => "Full Orichalcum",
        0x34 => "Orichalcum Frag (Right)",
        0x35 => "Orichalcum Frag (Bottom)",
        0x36 => "Orichalcum Frag (Left)",
        0x37 => "Golden Sun",
        0x38 => "Onyx Moonshard",
        0x39 => "Samsara",
        _ => "Undefined Item",
    }
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

/// This is for in world pickups only, i.e orbs, key items (Astro board), items on the ground (M2 Vital Star)
fn install_jump_rax_for_in_world(custom_function: usize) {
    // This is for Location checking
    unsafe {
        modify_call_offset(0x1b7143usize + get_dmc3_base_address(), 11); //sub
        modify_jmp_offset(0x1b5ADDusize + get_dmc3_base_address(), 11); //sub fixes key items as well...
        let target_address = get_dmc3_base_address() + TARGET_FUNCTION;
        // Step 1: Modify memory protection to allow writing
        let mut old_protect = 0;
        VirtualProtect(
            target_address as *mut _,
            16, // MOV + JMP = 12 bytes
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        // Step 2: Write the absolute jump (MOV RAX + JMP RAX)
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

        // Step 3: Restore the original memory protection
        VirtualProtect(target_address as *mut _, 16, old_protect, &mut old_protect);

        println!(
            "Installed absolute jump: Target Address = 0x{:x}, Custom Function = 0x{:x}",
            target_address, custom_function
        );
    }
}

/// Modifies a CALL instructions offset, subtracting it by the given value
fn modify_call_offset(call_address: usize, modify: i32) {
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

        // Ensure the opcode is CALL (0xE8)
        if call_code[0] != 0xE8 {
            panic!(
                "Instruction at 0x{:x} is not a CALL instruction. Opcode: 0x{:x}",
                call_address, call_code[0]
            );
        }

        // Extract the existing 4-byte relative offset
        let existing_offset = i32::from_le_bytes(call_code[1..5].try_into().unwrap());

        // Step 3: Calculate the new offset
        let new_offset = existing_offset.wrapping_sub(modify);

        // Step 4: Write the new offset
        call_code[1..5].copy_from_slice(&new_offset.to_le_bytes());

        // Step 5: Restore the original memory protection
        VirtualProtect(call_address as *mut _, 5, old_protect, &mut old_protect);

        println!(
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

        // Step 4: Write the new offset
        call_code[1..5].copy_from_slice(&new_offset.to_le_bytes());

        // Step 5: Restore the original memory protection
        VirtualProtect(call_address as *mut _, 5, old_protect, &mut old_protect);

        println!(
            "Modified JMP instruction at 0x{:x}: Old Offset = 0x{:x}, Modify = {}, New Offset = 0x{:x}",
            call_address, existing_offset, modify, new_offset
        );
    }
}

#[no_mangle]
pub unsafe extern "system" fn create_console() {
    unsafe {
        if AllocConsole().is_ok() {
            println!("Console created successfully!");
        } else {
            eprintln!("Failed to allocate console!");
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

fn input(text: &str) -> Result<String, anyhow::Error> {
    println!("{}", text);

    Ok(io::stdin().lock().lines().next().unwrap()?)
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

    match fdw_reason {
        DLL_PROCESS_ATTACH => unsafe {
            let rx = setup_channel();
            thread::Builder::new()
                .name("Archipelago Client".to_string())
                .spawn(move || {
                    create_console();
                    spawn_arch_thread(rx);
                    println!("Spawn thread...");
                })
                .expect("Failed to spawn arch thread");
            install_jump_rax_for_in_world(check_off_location as usize);
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
    println!("In thread");
    loop {
        if connected == false {
            println!("Going for connection");
            client = connect_archipelago().await;
            match &client {
                Ok(cl) => {
                    println!("Room Info: {:?}", cl.room_info());
                    connected = true
                }
                Err(..) => println!("Failed to connect"),
            }
        }
        match client {
            Ok(ref mut cl) => {
                if setup == false {
                    cl.status_update(ClientStatus::ClientConnected)
                        .await
                        .expect("Status update failed?");
                    run_setup(&cl);
                    setup = true;
                }
                handle_things(cl, &rx).await;
            }
            Err(..) => println!("Not connected?"),
        }
    }
}

fn run_setup(cl: &ArchipelagoClient) {
    println!("Running setup");
    match cl.data_package() {
        Some(dat) => {
            println!(
                "Item to ID: {:#?}",
                &dat.games["Devil May Cry 3"].item_name_to_id
            );
            println!(
                "Loc to ID: {:#?}",
                &dat.games["Devil May Cry 3"].location_name_to_id
            );
        }
        None => {}
    }
}

async fn handle_things(cl: &mut ArchipelagoClient, rx: &Arc<Mutex<Receiver<Location>>>) {
    if let Ok(rec) = rx.lock() {
        while let Ok(item) = rec.recv() {
            println!("Processing item: {}", item);
            //&cl.location_checks(vec![]).await;
        }
    }
    println!("Ready for receiving");
    match cl.recv().await {
        Ok(opt_msg) => match opt_msg {
            None => {
                println!("Received None for msg");
            }
            Some(msg) => {
                println!("Received message: {:?}", msg);
            }
        },
        Err(err) => {
            println!("Failed to receive data: {}", err)
        }
    }
}
// async fn disconnect_archipelago() {
//     match CLIENT {
//         Some(_client) => {
//             println!("Disconnecting from Archipelago server...");
//         }
//         None => {
//             println!("Not connected")
//         }
//     }
// }

async fn connect_archipelago() -> Result<ArchipelagoClient, anyhow::Error> {
    let url = input("Archipelago URL: ")?;
    println!("url: {}", url);

    let mut client: Result<ArchipelagoClient, ArchipelagoError> =
        Err(ArchipelagoError::ConnectionClosed);
    if !check_for_cache_file() {
        client = ArchipelagoClient::with_data_package(&url, Some(vec!["Devil May Cry 3".parse()?])).await;
        match &client {
            Ok(cl) => match &cl.data_package() {
                None => return Err(anyhow!("Data package does not exist")),
                Some(ref dp) => {
                    let mut clone_data = HashMap::new();
                    let _ = &dp.games.iter().for_each(|g| {
                        let dat = CoolGameData {
                            item_name_to_id: g.1.item_name_to_id.clone(),
                            location_name_to_id: g.1.location_name_to_id.clone(),
                        };
                        clone_data.insert(g.0.clone(), dat);
                    });
                    write_cache(clone_data, cl.room_info())
                        .await
                        .expect("Shit fucked up!");
                }
            },
            Err(er) => return Err(anyhow!("Failed to connect to (Data) Archipelago: {}", er)),
        }
    } else {
        client = ArchipelagoClient::new(&url).await;
        match client {
            Ok(ref mut cl) => {
                let option = check_checksums(cl.room_info()).await;
                match option {
                    None => println!("Checksums check out!"),
                    Some(failures) => {
                        println!("Checksums check failures: {:?}", failures);
                        remove_file("cache.json")?;
                        client = Err(ArchipelagoError::ConnectionClosed);
                        return Err(anyhow!("Reconnecting to grab cache!"));
                    }
                }
            }
            Err(er) => return Err(anyhow!("Failed to connect to Archipelago: {}", er)),
        }
    }
    let name = input("Name: ")?;
    let password = input("Password (Leave blank if unneeded): ")?;
    println!("Connecting to url");
    match client {
        // Whether we have a client
        Ok(mut cl) => {
            println!("Attempting room connection");
            let res = cl.connect(
                "Devil May Cry 3",
                &name,
                Some(&password),
                Option::from(0b101),
                vec!["AP".to_string()],
                true,
            );
            match res.await {
                Ok(stat) => {
                    println!("Connected info: {:?}", stat);
                    Ok(cl)
                }
                _err => Err(anyhow!("Failed to connect to room")),
            }
        }

        _ => Err(anyhow!("Failed to connect to server")),
    }
}

/// Checks for the Archipelago RoomInfo cache file
/// If file exists then check the checksums in it
/// Returns false if file doesn't exist (or if it cant be checked for)
fn check_for_cache_file() -> bool {
    match Path::new("cache.json").try_exists() {
        Ok(res) => {
            if res == true {
                println!("Cache file Exists!");
                true
            } else {
                false
            }
        }
        Err(_) => {
            println!("Failed to check for cache file!");
            false
        }
    }
}

#[derive(Deserialize, Serialize)]
struct Cache {
    checksums: HashMap<String, String>,
    data_package: HashMap<String, CoolGameData>,
}

/// Check the cached checksums with the stored file. Return any that do not match
async fn check_checksums(room_info: &RoomInfo) -> Option<Vec<String>> {
    let file = File::open("cache.json");
    match file {
        Ok(cache) => {
            let reader = BufReader::new(cache);
            let mut json_reader = serde_json::Deserializer::from_reader(reader);
            let json = Cache::deserialize(&mut json_reader);
            match json {
                Ok(cac) => {
                    let mut failed_checks = vec![];
                    for key in cac.checksums.keys() {
                        if room_info.datapackage_checksums.get(key)
                            != cac.checksums.get(key.as_str())
                        {
                            failed_checks.push(key.clone());
                        }
                    }
                    if failed_checks.is_empty() {
                        None
                    } else {
                        Some(failed_checks)
                    }
                }
                _err => None, // TODO ?
            }
        }
        Err(_) => None,
    }
}

#[derive(Deserialize, Serialize)]
pub struct CloneableData(GameData);

#[derive(Deserialize, Serialize)]
pub struct CoolGameData {
    pub item_name_to_id: HashMap<String, i32>,
    pub location_name_to_id: HashMap<String, i32>,
}

/// Write the DataPackage to a JSON file
async fn write_cache(
    data: HashMap<String, CoolGameData>,
    room_info: &RoomInfo,
) -> Result<(), anyhow::Error> {
    let mut file = File::create("cache.json")?;
    // let mut game_data = HashMap::new();
    // game_data.clone_from(&mut data);
    let cache: Cache = Cache {
        checksums: room_info.datapackage_checksums.clone(),
        data_package: data,
    };
    // let string = serde_json::to_string(&cache)?;
    file.write_all(serde_json::to_string_pretty(&cache)?.as_bytes())?;
    file.flush()?;
    println!("Writing cache");
    Ok(())
}

// async fn perform_connection(url: String) -> Result<ArchipelagoClient, ArchipelagoError> {
//     let result = ArchipelagoClient::new(&url).await;
//     result
//     // match result {
//     //     Ok(res) => Some(res),
//     //     Err(err) => { println!("{}", err); None },
//     // }
// }
