use crate::utilities::get_dmc3_base_address;

// TODO Replace loaded save file with archipelago specific one
const SAVE_NAME_ADDR: usize = 0x3711b8;

fn edit_save_name(new_name: String) {
    unsafe { std::ptr::copy(new_name.as_ptr(), (SAVE_NAME_ADDR + get_dmc3_base_address()) as *mut u8, new_name.bytes().len()); }
}

fn get_save_name(seed: String) -> String {
    let mut copied = seed.clone();
    copied.truncate(6);
    format!("dmc3{}.sav", copied)
}

fn create_special_save(new_name: String) {
    todo!()
}