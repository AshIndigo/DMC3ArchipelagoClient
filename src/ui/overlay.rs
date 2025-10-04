use std::slice::from_raw_parts;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;
use crate::constants::Status;
use crate::ui::font_handler::{FontAtlas, FontColorCB};
use crate::ui::ui::CONNECTION_STATUS;
use crate::ui::{dx11_hooks, font_handler};
use crate::utilities;
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

static FONT_ATLAS: OnceLock<FontAtlas> = OnceLock::new();

static INPUT_LAYOUT: OnceLock<ID3D11InputLayout> = OnceLock::new();

static VERTEX_BUFFER: OnceLock<ID3D11Buffer> = OnceLock::new();

static RTV: OnceLock<ID3D11RenderTargetView> = OnceLock::new();
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

pub static MESSAGE_QUEUE: LazyLock<Vec<OverlayMessage>> = LazyLock::new(|| vec![]);
pub struct OverlayMessage {
    text: String,
    color: FontColorCB,
    duration: Duration,
    x: f32,
    y: f32,
}

fn get_resources(
    swap_chain: &IDXGISwapChain,
) -> (
    &ID3D11RenderTargetView,
    &ID3D11Buffer,
    &FontAtlas,
    &ID3D11InputLayout,
) {
    let device: ID3D11Device = unsafe { swap_chain.GetDevice() }.unwrap();
    let rtv = RTV.get_or_init(|| {
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
    });
    let vertex_buffer = VERTEX_BUFFER.get_or_init(|| {
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
    });
    let atlas = FONT_ATLAS.get_or_init(|| {
        const FONT_SIZE: f32 = 36.0;
        const ROW_WIDTH: u32 = 256;
        let chars: Vec<char> = (0u8..=127).map(|c| c as char).collect();
        font_handler::create_rgba_font_atlas(&device, &*chars, FONT_SIZE, ROW_WIDTH).unwrap()
    });
    let input_layout = INPUT_LAYOUT.get_or_init(|| {
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
    });
    (rtv, vertex_buffer, atlas, input_layout)
}

pub(crate) unsafe fn present_hook(
    orig_swap_chain: IDXGISwapChain,
    sync_interval: u32,
    flags: u32,
) -> i32 {
    // It bothers me that this is called twice, but not sure how to fix it atm
    let device: ID3D11Device = unsafe { orig_swap_chain.GetDevice() }.unwrap();
    let context = unsafe { device.GetImmediateContext() }.unwrap();
    let (rtv, vertex_buffer, atlas, input_layout) = get_resources(&orig_swap_chain);
    let (screen_width, screen_height) = {
        let mut rect = RECT::default();
        unsafe { GetClientRect(orig_swap_chain.GetDesc().unwrap().OutputWindow, &mut rect) }
            .expect("Failed to get ClientRect");
        (
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        )
    };

    unsafe {
        context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: screen_width,
            Height: screen_height,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]));
    }

    /*
    TODO:
        - Want to have some info on the main menu (Connection Status, Version) - Hide when off main menu
        - Use this to display received items (Really should try to hide it if it's your own item.
     */
    font_handler::draw_string(
        &context,
        &vertex_buffer,
        &input_layout,
        &atlas,
        format!(
            "Status: {}",
            Status::from_repr(CONNECTION_STATUS.load(Ordering::SeqCst) as usize).unwrap()
        ),
        0.0,
        0.0,
        screen_width,
        screen_height,
        FontColorCB {
            color: [1.0, 0.0, 0.0, 1.0],
        },
    );

    font_handler::draw_string(
        &context,
        &vertex_buffer,
        &input_layout,
        &atlas,
        format!("Main Menu: {}", utilities::is_on_main_menu()),
        0.0,
        80.0,
        screen_width,
        screen_height,
        FontColorCB {
            color: [0.0, 0.0, 1.0, 1.0],
        },
    );

    unsafe { dx11_hooks::ORIGINAL_PRESENT.get().unwrap()(orig_swap_chain, sync_interval, flags) }
}
