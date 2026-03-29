use crate::archipelago::CONNECTED;
use crate::utilities::is_crimson_loaded;
use crate::{mapping, utilities};
use archipelago_rs::LocatedItem;
use randomizer_utilities::dmc::loader_parser::LOADER_STATUS;
use randomizer_utilities::ui::dx11::{ORIGINAL_PRESENT, ORIGINAL_RESIZE_BUFFERS};
use randomizer_utilities::ui::font_handler::{BLACK, FontAtlas, FontColorCB, GREEN, RED, WHITE};
use randomizer_utilities::ui::overlay::{D3D11State, STATE};
use randomizer_utilities::ui::{font_handler, overlay};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, RwLockReadGuard};
use std::time::{Duration, Instant};
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::Interface;

static MESSAGE_QUEUE: LazyLock<Mutex<VecDeque<OverlayMessage>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

static ACTIVE_MESSAGES: LazyLock<Mutex<VecDeque<TimedMessage>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

pub struct MessageSegment {
    pub text: String,
    pub color: FontColorCB,
}

impl MessageSegment {
    pub fn new(text: String, color: FontColorCB) -> Self {
        Self { text, color }
    }
}
pub struct OverlayMessage {
    segments: Vec<MessageSegment>,
    duration: Duration,
    _x: f32,
    _y: f32,
    _msg_type: MessageType,
}

impl OverlayMessage {
    pub(crate) fn new(
        segments: Vec<MessageSegment>,
        duration: Duration,
        x: f32,
        y: f32,
        msg_type: MessageType,
    ) -> OverlayMessage {
        OverlayMessage {
            segments,
            duration,
            _x: x,
            _y: y,
            _msg_type: msg_type,
        }
    }
}
// TODO This doesn't matter right now, but it could be used later
pub(crate) enum MessageType {
    _Default,     // Take the X and Y values as they are given
    Notification, // Disregard coordinates, automatically align to upper right (Used for newly received items+DL)
}

pub fn get_default_color() -> &'static FontColorCB {
    if is_crimson_loaded() { &WHITE } else { &BLACK }
}

pub(crate) fn add_message(overlay: OverlayMessage) {
    match MESSAGE_QUEUE.lock() {
        Ok(mut queue) => {
            queue.push_back(overlay);
        }
        Err(err) => {
            log::error!("PoisonError upon trying to add message {:?}", err);
        }
    }
}

pub(crate) unsafe extern "system" fn resize_hook(
    swap_chain: *mut IDXGISwapChain,
    buffer_count: u32,
    width: u32,
    height: u32,
    new_format: Common::DXGI_FORMAT,
    swap_chain_flags: DXGI_SWAP_CHAIN_FLAG,
) {
    unsafe {
        ORIGINAL_RESIZE_BUFFERS.get().unwrap()(
            swap_chain,
            buffer_count,
            width,
            height,
            new_format,
            swap_chain_flags,
        )
    };
    if let Some(state) = STATE.get() {
        match state.write() {
            Ok(mut state) => {
                state.rtv = None;
                state.atlas = None;
            }
            Err(err) => {
                log::error!("Unable to edit D3D11State {}", err)
            }
        }
    }
}

pub(crate) static CANT_PURCHASE: AtomicBool = AtomicBool::new(false);

unsafe fn update_screen_size(swap_chain: &IDXGISwapChain) -> (f32, f32) {
    let back_buffer: ID3D11Texture2D = {
        let ptr: ID3D11Texture2D =
            unsafe { swap_chain.GetBuffer(0) }.expect("Failed to get back buffer");
        ptr.cast().unwrap()
    };

    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe {
        back_buffer.GetDesc(&mut desc);
    }

    (desc.Width as f32, desc.Height as f32)
}

pub(crate) unsafe extern "system" fn present_hook(
    orig_swap_chain: IDXGISwapChain,
    sync_interval: u32,
    flags: u32,
) -> i32 {
    let (screen_width, screen_height) = unsafe { update_screen_size(&orig_swap_chain) };
    let state = overlay::get_resources(&orig_swap_chain);
    match state.read() {
        Ok(state) => {
            unsafe {
                state
                    .context
                    .OMSetRenderTargets(Some(std::slice::from_ref(&state.rtv)), None);
                state.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                    TopLeftX: 0.0,
                    TopLeftY: 0.0,
                    Width: screen_width,
                    Height: screen_height,
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                }]));
            }

            if (utilities::is_on_main_menu() || should_display_anyway())
                && let Some(atlas) = &state.atlas
            {
                const STATUS: &str = "Status: ";
                font_handler::draw_string(
                    &state,
                    STATUS,
                    0.0,
                    0.0,
                    screen_width,
                    screen_height,
                    get_default_color(),
                );
                let connected = CONNECTED.load(Ordering::SeqCst);
                font_handler::draw_string(
                    &state,
                    if connected {
                        "Connected"
                    } else {
                        "Disconnected"
                    },
                    STATUS.chars().map(|c| atlas.glyph_advance(c)).sum::<f32>(),
                    0.0,
                    screen_width,
                    screen_height,
                    &if connected { GREEN } else { RED },
                );
                draw_version_info(&state, screen_width, screen_height, atlas);
            }
            if CANT_PURCHASE.load(Ordering::SeqCst)
                && let Some(atlas) = &state.atlas
            {
                // TODO Modify this text
                const NO_PURCHASE: &str = "Cannot purchase upgrades";
                const NO_PURCHASE_L2: &str = "due to world settings";
                font_handler::draw_string(
                    &state,
                    NO_PURCHASE,
                    480.0
                        + (NO_PURCHASE
                            .chars()
                            .map(|c| atlas.glyph_advance(c))
                            .sum::<f32>()
                            / 2.0),
                    70.0,
                    screen_width,
                    screen_height,
                    &WHITE,
                );
                font_handler::draw_string(
                    &state,
                    NO_PURCHASE_L2,
                    480.0
                        + (NO_PURCHASE
                            .chars()
                            .map(|c| atlas.glyph_advance(c))
                            .sum::<f32>()
                            / 2.0),
                    106.0,
                    screen_width,
                    screen_height,
                    &WHITE,
                );
                CANT_PURCHASE.store(false, Ordering::SeqCst);
            }

            pop_buffer_message();

            let now = Instant::now();
            if let Ok(mut active) = ACTIVE_MESSAGES.lock() {
                // If it hasn't expired, keep it around
                active.retain(|msg| msg.expiration > now);

                const PADDING: f32 = 12.0;
                const LINE_HEIGHT: f32 = 24.0;

                let mut y = PADDING;
                for msg in active.iter().rev() {
                    draw_colored_message(&state, msg, screen_width, screen_height, y);
                    y += LINE_HEIGHT + PADDING;
                }
            }
        }
        Err(err) => {
            log::error!("Failed to get resources: {:?}", err);
        }
    }

    unsafe { ORIGINAL_PRESENT.get().unwrap()(orig_swap_chain, sync_interval, flags) }
}

fn draw_version_info(
    state: &RwLockReadGuard<D3D11State>,
    screen_width: f32,
    screen_height: f32,
    atlas: &FontAtlas,
) {
    const MOD_VERSION: &str = "Mod Version:";
    const AP_VERSION: &str = "AP Client Version:";
    const ROOM_VERSION: &str = "Room Version:";
    const MODE: &str = "Mode:";
    const GAME_VERSION: &str = "Game Version:";
    const ADDITIONAL_MODS: &str = "Additional Mods:";
    // TODO Maybe at some point I'd want to have the mod poke github on launch?
    font_handler::draw_string(
        state,
        &format!("{} {}", MOD_VERSION, env!("CARGO_PKG_VERSION")),
        0.0,
        //VERSION.chars().map(|c| atlas.glyph_advance(c)).sum::<f32>(),
        100.0,
        screen_width,
        screen_height,
        get_default_color(),
    );

    if CONNECTED.load(Ordering::SeqCst)
        && let Ok(mapping) = mapping::OVERLAY_INFO.read()
    {
        font_handler::draw_string(
            state,
            &format!("{} {}", MODE, mapping.mode),
            0.0,
            //VERSION.chars().map(|c| atlas.glyph_advance(c)).sum::<f32>(),
            50.0,
            screen_width,
            screen_height,
            get_default_color(),
        );
        if let Some(cv) = &mapping.client_version {
            font_handler::draw_string(
                state,
                &format!("{} {}", AP_VERSION, cv),
                0.0,
                //VERSION.chars().map(|c| atlas.glyph_advance(c)).sum::<f32>(),
                150.0,
                screen_width,
                screen_height,
                get_default_color(),
            );
        }
        if let Some(gv) = &mapping.generated_version {
            font_handler::draw_string(
                state,
                &format!("{} {}", ROOM_VERSION, gv),
                0.0,
                //VERSION.chars().map(|c| atlas.glyph_advance(c)).sum::<f32>(),
                200.0,
                screen_width,
                screen_height,
                get_default_color(),
            );
        }
    }
    if let Some(status) = LOADER_STATUS.get() {
        font_handler::draw_string(
            state,
            GAME_VERSION,
            0.0,
            250.0,
            screen_width,
            screen_height,
            get_default_color(),
        );
        font_handler::draw_string(
            state,
            &format!(" {}", status.game_information.description),
            GAME_VERSION
                .chars()
                .map(|c| atlas.glyph_advance(c))
                .sum::<f32>(),
            250.0,
            screen_width,
            screen_height,
            if status.game_information.valid_for_use {
                &GREEN
            } else {
                &RED
            },
        );
        font_handler::draw_string(
            state,
            ADDITIONAL_MODS,
            0.0,
            300.0,
            screen_width,
            screen_height,
            get_default_color(),
        );
        for (i, mod_info) in status.mod_information.iter().enumerate() {
            let base = 350;
            font_handler::draw_string(
                state,
                mod_info.description,
                0.0,
                (base + (i * 50)) as f32,
                screen_width,
                screen_height,
                if mod_info.valid_for_use { &GREEN } else { &RED },
            );
        }
    }
}

fn should_display_anyway() -> bool {
    // TODO Use this to display if we are connected, then disconnected
    // Or if version mismatch?

    false
}

fn draw_colored_message(
    state: &D3D11State,
    msg: &TimedMessage,
    screen_width: f32,
    screen_height: f32,
    y: f32,
) {
    const FALLBACK_MULT: f32 = 32.0;
    let total_width: f32 = msg
        .message
        .segments
        .iter()
        .map(|seg| {
            if let Some(atlas) = &state.atlas {
                seg.text
                    .chars()
                    .map(|c| atlas.glyph_advance(c))
                    .sum::<f32>()
            } else {
                seg.text.len() as f32 * FALLBACK_MULT
            }
        })
        .sum();

    // Start on right
    let mut cursor_x = screen_width - total_width;

    for segment in msg.message.segments.iter() {
        font_handler::draw_string(
            state,
            &segment.text,
            cursor_x,
            y,
            screen_width,
            screen_height,
            &segment.color,
        );

        if let Some(atlas) = &state.atlas {
            cursor_x += segment
                .text
                .chars()
                .map(|c| atlas.glyph_advance(c))
                .sum::<f32>();
        } else {
            cursor_x += segment.text.len() as f32 * FALLBACK_MULT;
        }
    }
}

struct TimedMessage {
    message: OverlayMessage,
    expiration: Instant,
}

fn pop_buffer_message() {
    if let Ok(mut queue) = MESSAGE_QUEUE.lock()
        && let Some(message) = queue.pop_front()
    {
        let expiration = Instant::now() + message.duration;
        let timed = TimedMessage {
            message,
            expiration,
        };
        if let Ok(mut active) = ACTIVE_MESSAGES.lock() {
            active.push_back(timed);
        }
    }
}

pub(crate) fn get_color_for_item(item: &LocatedItem) -> FontColorCB {
    const CYAN: FontColorCB = FontColorCB::new(0.0, 0.933, 0.933, 1.0);
    const PLUM: FontColorCB = FontColorCB::new(0.686, 0.6, 0.937, 1.0);
    const STATE_BLUE: FontColorCB = FontColorCB::new(0.427, 0.545, 0.91, 1.0);
    const SALMON: FontColorCB = FontColorCB::new(0.98, 0.502, 0.447, 1.0);

    match (item.is_trap(), item.is_useful(), item.is_progression()) {
        (true, _, _) => SALMON,
        (false, _, true) => PLUM,
        (false, true, false) => STATE_BLUE,
        (false, false, false) => CYAN,
    }
}
