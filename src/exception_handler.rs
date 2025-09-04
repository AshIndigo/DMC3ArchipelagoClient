use winapi::um::errhandlingapi::AddVectoredExceptionHandler;
use winapi::um::winnt::EXCEPTION_POINTERS;

use std::{ffi::c_void, path::Path};
use winapi::shared::minwindef::{HINSTANCE, HMODULE};
use winapi::um::libloaderapi::{
    GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
};

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
        _ => "Unknown Exception",
    }
}

/// Resolve address → (module base, filename)
unsafe fn module_from_address(addr: *const c_void) -> Option<(HINSTANCE, String)> {
    let mut hmod = HMODULE::default();
    unsafe {
        if GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            addr as *const u16,
            &mut hmod,
        ) == 0
        {
            return None;
        }
    }

    let mut buf = [0u16; 260]; // MAX_PATH
    unsafe {
        let len = GetModuleFileNameW(hmod, buf.as_mut_ptr(), buf.len() as u32);
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
        let record = (*info).ExceptionRecord;
        if record.is_null() {
            return 0;
        }

        let code = (*record).ExceptionCode;
        let address = (*record).ExceptionAddress as usize;
        if code < 0x80000000 {
            // Don't care
            return 0;
        }

        // Get the module+offset
        if let Some((base, name)) = module_from_address(address as *const c_void) {
            let offset = address - base as usize;
            log::error!(
                "Exception {:#X} ({}) at {:#p} in {}+0x{:X}",
                code,
                exception_code_to_str(code),
                address as *const c_void,
                name,
                offset
            );
        } else {
            log::error!(
                "Exception {:#X} ({}) at {:#p} (module unknown)",
                code,
                exception_code_to_str(code),
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
}
