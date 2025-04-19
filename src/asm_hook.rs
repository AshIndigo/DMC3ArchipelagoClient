use std::arch::asm;
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::winnt::PAGE_EXECUTE_READWRITE;
use std::slice;
use crate::archipelago::MAPPING;
use crate::{generated_locations, hook, constants, utilities};
use crate::hook::{Location, TX};

const TARGET_FUNCTION: usize = 0x1b4595;

/// Unused, possibly to be deleted
unsafe fn modify_itm() {
    //noinspection RsBorrowChecker
    unsafe fn modify_itm_memory() {
        let itm_addr: *mut i32;
        let item_id: u32;
        asm!(
            "lea edx, [rcx+0x10]",
            "mov eax, [edx]",
            out("edx") itm_addr,
            out("eax") item_id, // TODO would be cool to reduce this even more
            // TODO This doesn't work for ITM files that have multiple items and the one we want to change is not the 1st item (ex. Room 5)
            clobber_abi("win64")
        );
        crate::hook::modify_adjudicator_drop(); // Should be fine right here?
        match MAPPING.get() {
            Some(mapping) => {
                let room_num: u16 = crate::utilities::get_room() as u16; // read_int_from_address(0xC8F258usize) as u16;
                for (location_name, entry) in generated_locations::ITEM_MISSION_MAP.iter() {
                    if entry.room_number == 0 {
                        // Skipping if location file has room as 0, that means its either event or not done
                        continue;
                    }
                    //log::debug!("Room number X: {} Room number memory: {}, Item ID X: 0x{:x}, Item ID Memory: 0x{:x}", entry.room_number, room_num, entry.item_id, item_id);
                    if entry.room_number == room_num && entry.item_id as u32 == item_id {
                        let ins_val = constants::get_item_id(mapping.items.get(*location_name).unwrap()); // Scary
                        *itm_addr = ins_val.unwrap() as i32;
                        log::info!(
                            "Replaced item in room {} ({}) with 0x{:x}",
                            entry.room_number,
                            location_name,
                            ins_val.unwrap() as i32
                        );
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
        if item_id > 0x02u64 {
            if let Some(tx) = TX.get() {
                tx.send(Location {
                    item_id, // This is fine
                    room: utilities::get_room(),
                    mission: utilities::get_mission(),
                    room_5: false, // TODO
                })
                    .expect("Failed to send Location!");
            }
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

#[allow(dead_code)] /// Remains unused for now
pub(crate) fn install_jmps() {
    log::info!("Installing trampoline for in-world items");
    install_jump_rax_for_in_world(check_off_location as usize);
    log::info!("Installing trampoline for modifying itm files");
    install_jump_rax_for_itm_file(modify_itm as usize);
}

fn install_jump_rax_for_itm_file(custom_function: usize) {
    // This is for Location checking
    unsafe {
        modify_call_offset(0x23ba41usize + utilities::get_dmc3_base_address(), 13); //sub
        modify_call_offset(0x23ce70usize + utilities::get_dmc3_base_address(), 13); //sub fixes key items as well...
        let target_address = utilities::get_dmc3_base_address() + 0x1B4433usize;
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
            target_address,
            custom_function
        );
    }
}

/// This is for in world pickups only, i.e orbs, key items (Astro board), items on the ground (M2 Vital Star)
fn install_jump_rax_for_in_world(custom_function: usize) {
    // This is for Location checking
    unsafe {
        modify_call_offset(0x1b7143usize + utilities::get_dmc3_base_address(), 11); //sub
        modify_jmp_offset(0x1b5ADDusize + utilities::get_dmc3_base_address(), 11); //sub fixes key items as well...
        let target_address = utilities::get_dmc3_base_address() + TARGET_FUNCTION;
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
            target_address,
            custom_function
        );
    }
}

/// Modifies a JMP instructions offset, subtracting it by the given value
pub(crate) fn modify_jmp_offset(call_address: usize, modify: i32) {
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

// Still being used VVV

// Due to the function I'm tacking this onto, need a double trampoline
pub(crate) fn install_super_jmp_for_events(custom_function: usize) {
    // This is for Location checking
    unsafe {
        modify_call_offset(0x1af0f8usize + utilities::get_dmc3_base_address(), 6); //sub, orig 6
        let first_target_address = utilities::get_dmc3_base_address() + 0x1A9BBAusize; // This is for the 6 bytes above 1a9bc0
        let mut old_protect = 0;
        let length_first = 6;
        VirtualProtect(
            // TODO Make a generic protection handler! remove duped code
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
        target_code[5] = 0x90;

        // Restore the original memory protection
        VirtualProtect(
            first_target_address as *mut _,
            length_first,
            old_protect,
            &mut old_protect,
        );

        log::debug!(
            "Installed 1st trampoline: Target Address = 0x{:x}, Custom Function = 0x{:x}",
            first_target_address,
            custom_function
        );

        let second_target_address = utilities::get_dmc3_base_address() + 0x1A9A41usize;
        let mut old_protect = 0;
        let length_second = 16;
        VirtualProtect(
            second_target_address as *mut _,
            length_second, // MOV + JMP = 12 bytes
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        );

        // Write the absolute jump (MOV RAX + JMP RAX)
        let target_code_second =
            slice::from_raw_parts_mut(second_target_address as *mut u8, length_second);
        target_code_second[0] = 0x50; // Push RAX
        target_code_second[1] = 0x48; // REX.W
        target_code_second[2] = 0xB8; // MOV RAX, imm64
        target_code_second[3..11].copy_from_slice(&custom_function.to_le_bytes());

        target_code_second[11] = 0xFF; // JMP opcode
        target_code_second[12] = 0xD0; // JMP RAX
        target_code_second[13] = 0x58; // POP Rax
        target_code_second[14] = 0xC3; // RET
        // Restore the original memory protection
        VirtualProtect(
            second_target_address as *mut _,
            length_second,
            old_protect,
            &mut old_protect,
        );
    }
}

/// Modifies a CALL instructions offset, subtracting it by the given value
pub(crate) fn modify_call_offset(call_address: usize, modify: i32) {
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