use crate::constants::Status;
use crate::ui::font_handler::{FontAtlas, FontColorCB, GREEN, RED, WHITE};
use crate::ui::ui::CONNECTION_STATUS;
use crate::ui::{dx11_hooks, font_handler};
use crate::utilities;
use std::collections::VecDeque;
use std::slice::from_raw_parts;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};
use windows::core::PCSTR;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R32G32B32_FLOAT, DXGI_FORMAT_R32G32_FLOAT,
};
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

pub(crate) struct D3D11State {
    device: ID3D11Device,
    pub(crate) context: ID3D11DeviceContext,
    pub(crate) atlas: Option<FontAtlas>,
    pub(crate) input_layout: ID3D11InputLayout,
    pub(crate) vertex_buffer: ID3D11Buffer,
    pub(crate) rtv: Option<ID3D11RenderTargetView>,
}

static STATE: OnceLock<RwLock<D3D11State>> = OnceLock::new();

pub(crate) static SHADERS: LazyLock<(ID3DBlob, ID3DBlob)> = LazyLock::new(|| {
    let mut vs_blob: Option<ID3DBlob> = None;
    let mut ps_blob: Option<ID3DBlob> = None;
    let mut err_blob: Option<ID3DBlob> = None;

    let vs_bytes = include_bytes!(".././data/text_vs.hlsl");
    let ps_bytes = include_bytes!(".././data/text_ps.hlsl");

    unsafe {
        D3DCompile(
            vs_bytes.as_ptr() as *const _,
            vs_bytes.len(),
            None,
            None,
            None,
            PCSTR::from_raw("main\0".as_ptr()),
            PCSTR::from_raw("vs_5_0\0".as_ptr()),
            0,
            0,
            &mut vs_blob,
            Some(&mut err_blob),
        )
        .expect("Couldn't compile VS");
        D3DCompile(
            ps_bytes.as_ptr() as *const _,
            ps_bytes.len(),
            None,
            None,
            None,
            PCSTR::from_raw("main\0".as_ptr()),
            PCSTR::from_raw("ps_5_0\0".as_ptr()),
            0,
            0,
            &mut ps_blob,
            Some(&mut err_blob),
        )
        .expect("Couldn't compile PS");
    }
    (ps_blob.unwrap(), vs_blob.unwrap())
});

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
    x: f32,
    y: f32,
    msg_type: MessageType,
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
            x,
            y,
            msg_type,
        }
    }
}
// TODO This doesn't matter right now, but it could be used later
pub(crate) enum MessageType {
    Default,  // Take the X and Y values as they are given
    Notification, // Disregard coordinates, automatically align to upper right (Used for newly received items+DL)
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

fn get_rtv_atlas(
    device: &ID3D11Device,
    swap_chain: &IDXGISwapChain,
) -> (ID3D11RenderTargetView, FontAtlas) {
    let atlas = {
        const FONT_SIZE: f32 = 36.0;
        const ROW_WIDTH: u32 = 256;
        let chars: Vec<char> = (0u8..=127).map(|c| c as char).collect();
        font_handler::create_rgba_font_atlas(&device, &*chars, FONT_SIZE, ROW_WIDTH).unwrap()
    };

    let rtv = {
        let mut rtv = None;
        if let Err(e) = unsafe {
            device.CreateRenderTargetView(
                &swap_chain.GetBuffer::<ID3D11Texture2D>(0).unwrap(),
                None,
                Some(&mut rtv),
            )
        } {
            log::error!("Failed to create RTV: {:?}", e);
        }
        rtv.unwrap()
    };
    (rtv, atlas)
}

fn get_resources(swap_chain: &IDXGISwapChain) -> &RwLock<D3D11State> {
    let state = STATE.get_or_init(|| {
        let device: ID3D11Device = unsafe { swap_chain.GetDevice() }.unwrap();
        let vertex_buffer = {
            const VERTEX_BUFFER_DESC: D3D11_BUFFER_DESC = D3D11_BUFFER_DESC {
                ByteWidth: (size_of::<font_handler::Vertex>() * 4 * 256) as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                MiscFlags: 0,
                StructureByteStride: 0,
            };

            let mut vertex_buffer = None;
            if let Err(e) =
                unsafe { device.CreateBuffer(&VERTEX_BUFFER_DESC, None, Some(&mut vertex_buffer)) }
            {
                log::error!("Failed to create RTV: {:?}", e);
            }
            vertex_buffer.unwrap()
        };
        let input_layout = {
            const INPUT_ELEMENT_DESCS: [D3D11_INPUT_ELEMENT_DESC; 2] = [
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: PCSTR::from_raw(b"POSITION\0".as_ptr() as *const _),
                    SemanticIndex: 0,
                    Format: DXGI_FORMAT_R32G32B32_FLOAT,
                    InputSlot: 0,
                    AlignedByteOffset: 0,
                    InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                    InstanceDataStepRate: 0,
                },
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: PCSTR::from_raw(b"TEXCOORD\0".as_ptr() as *const _),
                    SemanticIndex: 0,
                    Format: DXGI_FORMAT_R32G32_FLOAT,
                    InputSlot: 0,
                    AlignedByteOffset: 12, // after the float3 position
                    InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                    InstanceDataStepRate: 0,
                },
            ];
            let (_, vsb) = &*SHADERS;
            let mut input_thingy = None;
            unsafe {
                device
                    .CreateInputLayout(
                        &INPUT_ELEMENT_DESCS,
                        from_raw_parts(vsb.GetBufferPointer() as *const u8, vsb.GetBufferSize()),
                        Some(&mut input_thingy),
                    )
                    .unwrap();
            }
            input_thingy.unwrap()
        };
        let (rtv, atlas) = get_rtv_atlas(&device, swap_chain);
        RwLock::new(D3D11State {
            device,
            context: unsafe {
                swap_chain
                    .GetDevice::<ID3D11Device>()
                    .unwrap()
                    .GetImmediateContext()
            }
            .unwrap(),
            atlas: Some(atlas),
            input_layout,
            vertex_buffer,
            rtv: Some(rtv),
        })
    });
    match state.write() {
        Ok(mut state) => {
            let (rtv, atlas) = get_rtv_atlas(&state.device, swap_chain);
            if state.rtv.is_none() {
                state.rtv = Some(rtv);
            }
            if state.atlas.is_none() {
                state.atlas = Some(atlas);
            }
        }
        Err(err) => {
            log::error!("PoisonError upon trying to write {:?}", err);
        }
    }
    state
}

pub(crate) unsafe fn resize_hook(
    swap_chain: *mut IDXGISwapChain,
    buffer_count: u32,
    width: u32,
    height: u32,
    new_format: Common::DXGI_FORMAT,
    swap_chain_flags: DXGI_SWAP_CHAIN_FLAG,
) {
    unsafe {
        dx11_hooks::ORIGINAL_RESIZE_BUFFERS.get().unwrap()(
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

pub(crate) unsafe fn present_hook(
    orig_swap_chain: IDXGISwapChain,
    sync_interval: u32,
    flags: u32,
) -> i32 {
    let (screen_width, screen_height) = {
        let mut rect = RECT::default();
        unsafe { GetClientRect(orig_swap_chain.GetDesc().unwrap().OutputWindow, &mut rect) }
            .expect("Failed to get ClientRect");
        (
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        )
    };
    let state = get_resources(&orig_swap_chain);
    match state.read() {
        Ok(state) => {
            unsafe {
                state
                    .context
                    .OMSetRenderTargets(Some(&[state.rtv.clone()]), None);
                state.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                    TopLeftX: 0.0,
                    TopLeftY: 0.0,
                    Width: screen_width,
                    Height: screen_height,
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                }]));
            }

            if utilities::is_on_main_menu() {
                if let Some(atlas) = &state.atlas {
                    const STATUS: &str = &"Status: ";
                    font_handler::draw_string(
                        &state,
                        &STATUS.to_string(),
                        0.0,
                        0.0,
                        screen_width,
                        screen_height,
                        &WHITE,
                    );
                    let status =
                        Status::from_repr(CONNECTION_STATUS.load(Ordering::SeqCst) as usize)
                            .unwrap();
                    font_handler::draw_string(
                        &state,
                        &format!("{}", status),
                        STATUS.chars().map(|c| atlas.glyph_advance(c)).sum::<f32>(),
                        0.0,
                        screen_width,
                        screen_height,
                        &match status {
                            Status::Connected => GREEN,
                            _ => RED,
                        },
                    );
                    const VERSION: &str = &"Version: ";
                    font_handler::draw_string(
                        &state,
                        &VERSION.to_string(),
                        0.0,
                        50.0,
                        screen_width,
                        screen_height,
                        &WHITE,
                    );

                    // TODO Maybe at some point I'd want to have the mod poke github on launch?
                    font_handler::draw_string(
                        &state,
                        &format!("{}", env!("CARGO_PKG_VERSION")),
                        VERSION.chars().map(|c| atlas.glyph_advance(c)).sum::<f32>(),
                        50.0,
                        screen_width,
                        screen_height,
                        &WHITE,
                    );
                }
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

    unsafe { dx11_hooks::ORIGINAL_PRESENT.get().unwrap()(orig_swap_chain, sync_interval, flags) }
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
    if let Ok(mut queue) = MESSAGE_QUEUE.lock() {
        if let Some(message) = queue.pop_front() {
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
}

pub(crate) fn get_color_for_item(flags: i32) -> FontColorCB {
    const CYAN: FontColorCB = FontColorCB::new(0.0, 0.933, 0.933, 1.0);
    const PLUM: FontColorCB = FontColorCB::new(0.686, 0.6, 0.937, 1.0);
    const STATE_BLUE: FontColorCB = FontColorCB::new(0.427, 0.545, 0.91, 1.0);
    const SALMON: FontColorCB = FontColorCB::new(0.98, 0.502, 0.447, 1.0);
    match flags {
        0b000 => CYAN,       // Cyan for regular/filler
        0b001 => PLUM,       // Plum for progression
        0b010 => STATE_BLUE, // 'Stateblue' for useful
        0b100 => SALMON,     // Salmon for trap
        0b101 => PLUM,       // Plum for progression
        0b110 => STATE_BLUE, // 'Stateblue' for useful
        0b011 => PLUM,       // Plum-gression (could be gold if I wanted to do proguseful)
        _ => WHITE,
    }
}
