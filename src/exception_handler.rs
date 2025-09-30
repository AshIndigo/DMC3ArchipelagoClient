

use std::{ffi::c_void, path::Path};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HMODULE};
use windows::Win32::System::Diagnostics::Debug::{AddVectoredExceptionHandler, EXCEPTION_POINTERS};
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS};

// There's probably more to add, but figured this would be good to start
fn exception_code_to_str(code: u32) -> &'static str {
    match code {
        0xC0000005 => "Access Violation",
        0x80000003 => "Breakpoint",
        0xC0000094 => "Integer Divide by Zero",
        0xC000001D => "Illegal Instruction",
        0xC000008E => "Floating-Point Divide by Zero",
        0xC0000096 => "Privileged Instruction",
        0xC00000FD => "Stack Overflow",
        0xC0000374 => "Heap Corruption",
        0xE06D7363 => "C++ Exception",
        _ => "Unknown Exception",
    }
}

/// Resolve address → (module base, filename)
unsafe fn module_from_address(addr: *const c_void) -> Option<(HMODULE, String)> {
    let mut hmod = HMODULE::default();
    unsafe {
        if GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            PCWSTR::from_raw( addr as *const u16),
            &mut hmod,
        ).is_err()
        {
            return None;
        }
    }

    let mut buf = [0u16; 260]; // MAX_PATH
    unsafe {
        let len = GetModuleFileNameW(Option::from(hmod), &mut buf);
        if len == 0 {
            return None;
        }

        let full_path = String::from_utf16_lossy(&buf[..len as usize]);
        let file_name = Path::new(&full_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<unknown>")
            .to_string();

        Some((hmod, file_name))
    }
}

unsafe extern "system" fn exception_handler(info: *mut EXCEPTION_POINTERS) -> i32 {
    if info.is_null() {
        return 0;
    }

    unsafe {
        let record = &*(*info).ExceptionRecord;

        let code = record.ExceptionCode;
        let address = record.ExceptionAddress as usize;
        if code.is_ok() {
            return 0;
        }

        if code.0 == 0xE06D7363u32 as i32 { // TODO, this is jank af
            log::error!("C++ Exception detected at {:?}", record.ExceptionAddress);

            // ExceptionInformation[0] is a magic number for C++ EH
            let magic = record.ExceptionInformation[0];
            let frame = record.ExceptionInformation[1] as *const ();
            let desc = record.ExceptionInformation[2] as *const ();

            log::debug!("Magic: {:#X}", magic);
            log::debug!("Frame: {:p}", frame);
            log::debug!("Desc: {:p}", desc);

            if !desc.is_null() {
                let type_name_ptr = *(desc as *const *const i8);
                if !type_name_ptr.is_null() {
                    if let Ok(cstr) = std::ffi::CStr::from_ptr(type_name_ptr).to_str() {
                        log::error!("C++ exception type: {}", cstr);
                    }
                }
            }
        }

        // Get the module+offset
        if let Some((base, name)) = module_from_address(address as *const c_void) {
            let offset = address - base.0 as usize;
            log::error!(
                "Exception {:#X} ({}) at {:#p} in {}+0x{:X}",
                code.0,
                exception_code_to_str(code.0 as u32),
                address as *const c_void,
                name,
                offset
            );
        } else {
            log::error!(
                "Exception {:#X} ({}) at {:#p} (module unknown)",
                code.0,
                exception_code_to_str(code.0 as u32),
                address as *const c_void
            );
        }
        log::error!(
            "Please upload the \"dmc3_rando_latest.log\" in your game's log folder to either Github or to the Archipelago game thread!"
        );
        let ctx = *(*info).ContextRecord;
        // I could probably do this better but oh well.
        log::debug!(
            "RAX={:#018x} RBX={:#018x} RCX={:#018x}",
            ctx.Rax,
            ctx.Rbx,
            ctx.Rcx
        );
        log::debug!(
            "RDX={:#018x} RSI={:#018x} RDI={:#018x}",
            ctx.Rdx,
            ctx.Rsi,
            ctx.Rdi
        );
        log::debug!(
            "RBP={:#018x} RSP={:#018x} RIP={:#018x}",
            ctx.Rbp,
            ctx.Rsp,
            ctx.Rip
        );
        log::debug!(
            "R8 ={:#018x} R9 ={:#018x} R10={:#018x}",
            ctx.R8,
            ctx.R9,
            ctx.R10
        );
        log::debug!(
            "R11={:#018x} R12={:#018x} R13={:#018x}",
            ctx.R11,
            ctx.R12,
            ctx.R13
        );
        log::debug!("R14={:#018x} R15={:#018x}", ctx.R14, ctx.R15);
    }

    0
}

pub fn install_exception_handler() {
    unsafe {
        AddVectoredExceptionHandler(1, Some(exception_handler));
    }
    log::debug!("Installed exception handler");
}
