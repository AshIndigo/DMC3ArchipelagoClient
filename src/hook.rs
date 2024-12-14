use anyhow::anyhow;
use archipelago_rs::client::{ArchipelagoClient, ArchipelagoError};
use once_cell::sync::OnceCell;
use std::arch::asm;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::io::{BufRead, Write};
use std::os::windows::ffi::OsStrExt;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::{io, thread};
use std::{ptr, slice};
use tokio::runtime::Runtime;
use winapi::shared::minwindef::{HINSTANCE, LPVOID};
use winapi::um::libloaderapi::{FreeLibrary, GetModuleHandleW};
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::PAGE_EXECUTE_READWRITE;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::Console::{AllocConsole, FreeConsole};

const TARGET_FUNCTION: usize = 0x1b4595;
//static mut LOCATION_LIST: VecDeque<Location> = VecDeque::new();

static TX: OnceCell<Sender<Location>> = OnceCell::new();
//static RX: OnceCell<Receiver<Location>> = OnceCell::new();

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
pub unsafe extern "system" fn check_off_location() {
    let item_id: u64;
    asm!(
    "sub rsp, 16", // Align stack to 16 bytes
    "push rcx",
    "push rdx",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    "mov {}, rdx",
    lateout(reg) item_id
    );
    if let Some(tx) = TX.get() {
        tx.send(Location {
            item_id,
            room: read_int_from_address(0xC8F258),
        })
        .expect("Failed to send Location!");
    }

    asm!(
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "add rsp, 16", // Restore original stack alignment
        "movzx r9d, byte ptr [rcx+0x60]"
    );
}

fn get_item(item_id: u64) -> &'static str {
    match item_id {
        0x00 => "Red Orb Smol",
        0x01 => "Gold Orb",
        0x02 => "Auto2 Orb",
        0x03 => "Red Orb - 100",
        0x04 => "1k Red Orb",
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
fn install_jump_for_location(custom_function: usize) {
    // This is for Location checking
    unsafe {
        modify_call_offset(0x1b7143usize + get_dmc3_base_address(), 11); //sub
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

        // JMP RAX
        target_code[11] = 0xFF; // JMP opcode
        target_code[12] = 0xD0; // JMP RAX
                                //target_code[13] = 0x58; // POP Rax
        for i in 13..16 {
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

// fn input(text: &str) -> String {
//     println!("{}", text);
//
//     match io::stdin().lock().lines().next() {
//         Some(x) => x.unwrap_or_else(|_| String::from("")),
//         None => String::from(""),
//     }
// }

fn input(text: &str) -> Result<String, anyhow::Error> {
    println!("{}", text);

    Ok(io::stdin().lock().lines().next().unwrap()?)
}

// fn input<T: FromStr>() -> Result<T, <T as FromStr>::Err> {
//     let mut input: String = String::with_capacity(64);
//
//     std::io::stdin()
//         .read_line(&mut input)
//         .expect("Input could not be read");
//
//     input.parse()
// }

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
            //create_console();
            // thread::Builder::new()
            //     .name("Archipelago Client".to_string())
            //     .spawn(async {
            //         create_console();
            //         println!("Spawn thread...");
            //         spawn_arch_thread(rx).await
            //     })
            //     .expect("Failed to spawn arch thread");
            thread::Builder::new()
                .name("Archipelago Client".to_string())
                .spawn(move || {
                    create_console();
                    //let runtime = Runtime::new().expect("Failed to create Tokio runtime");
                    //runtime.spawn(async {
                        spawn_arch_thread(rx);
                    //});
                    println!("Spawn thread...");
                })
                .expect("Failed to spawn arch thread");
            install_jump_for_location(check_off_location as usize);
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
#[tokio::main(flavor="current_thread")]
async unsafe fn spawn_arch_thread(rx: Arc<Mutex<Receiver<Location>>>) {
    let mut connected = false;
    let mut client = Err(anyhow::anyhow!("Archipelago Client not initialized"));
    println!("In thread");
    loop {
        if connected == false {
            println!("Going for connection");
            client = connect_archipelago().await;
            match &client {
                Ok(..) => connected = true,
                Err(..) => println!("Failed to connect"),
            }
        }
        match &client {
            Ok(cl) => handle_things(cl, &rx),
            Err(..) => println!("Not connected?"),
        }
    }
}

fn handle_things(cl: &ArchipelagoClient, rx: &Arc<Mutex<Receiver<Location>>>) {
    if let Ok(rec) = rx.lock() {
        while let Ok(item) = rec.recv() {
            println!("Processing item: {}", item);
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
    let mut client = ArchipelagoClient::new(&url).await; //perform_connection(url).await;
    let name = input("Name: ")?;
    let password = input("Password (Leave blank if unneeded): ")?;
    println!("Connecting to url");
    match client { // Whether we have a client
        Ok(mut cl) => {
            println!("Attempting room connection");
            let res = cl.connect("Devil May Cry 3", &name, Some(&password), Option::from(0b101), vec!["AP".to_string()], true);
            match res.await {
                Ok(_stat) => Ok(cl),
                _err => Err(anyhow!("Failed to connect to room")),
            }
        }

        _ => {Err(anyhow!("Failed to connect to server"))}
    }
}

async fn perform_connection(url: String) ->  Result<ArchipelagoClient, ArchipelagoError> {
    let result = ArchipelagoClient::new(&url).await;
    result
    // match result {
    //     Ok(res) => Some(res),
    //     Err(err) => { println!("{}", err); None },
    // }
}
