use crate::ui::ArchipelagoHud;
use minhook::MinHook;
use std::cell::RefCell;
use std::ops::DerefMut;

thread_local! {
    static HUD_INSTANCE: RefCell<ArchipelagoHud> = RefCell::new(ArchipelagoHud::new());
    static ORIG_FUNC: RefCell<Option<DdmkMainType >> = RefCell::new(None);
}

unsafe extern "C" fn hooked_render() {
    ORIG_FUNC.with(|orig_func| {
        orig_func();
    });
    match get_imgui_context() {
        Some(mut ctx) => {
            HUD_INSTANCE.with(|instance| {
                let mut hud = instance.borrow_mut();
                let ui = ctx.new_frame();
                crate::ui::render_imgui_ui(hud.deref_mut(), ui);
                ctx.render();
            });
        }
    _ => {}
}
}

fn get_imgui_context() -> Option<imgui::Context> {
    todo!()
}

type DdmkMainType = unsafe extern "system" fn() -> None;

unsafe fn setup_ddmk_hook() {
    let orig_main = MinHook::create_hook_api("Mary.dll", "Main", hooked_render as _)
        .expect("Failed to create DDMK hook");
    ORIG_FUNC.set(Some(std::mem::transmute::<_, DdmkMainType>(orig_main)));
}
