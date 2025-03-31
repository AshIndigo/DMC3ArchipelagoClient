#![feature(lock_value_accessors)]
#![recursion_limit = "512"]
mod hook;
mod cache;
mod archipelago;
mod constants;
mod tables;
mod hudhook_hook;
mod config;
mod ddmk_hook;
mod ui;
mod imgui_bindings;
mod inputs;
mod asm_hook;
mod utilities;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
