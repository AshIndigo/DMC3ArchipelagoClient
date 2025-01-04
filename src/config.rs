use imgui::Key;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub(crate) struct Settings {
    pub(crate) display: Key,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            display: Key::Tab,
        }
    }
}