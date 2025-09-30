use core::sync::atomic::Ordering;
use std::mem::ManuallyDrop;
use std::sync::atomic::AtomicBool;
use std::sync::OnceLock;

use crate::ui::dx11_hooks;
use crate::ui::dx11_hooks::{create_device_and_swap_chain, wide, Resources};
use windows::core::Interface;
use windows::core::PCWSTR;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
use windows::Win32::Graphics::Dxgi::*;

static DW_FACTORY: OnceLock<IDWriteFactory> = OnceLock::new();
static D2D_CONTEXT: OnceLock<ID2D1DeviceContext> = OnceLock::new();
static RESOURCES: OnceLock<Resources> = OnceLock::new();

static IN_OVERLAY_PRESENT: AtomicBool = AtomicBool::new(false);

fn setup_direct_write_devices() -> Result<(), Box<dyn std::error::Error>> {
    let d2d_factory: ID2D1Factory1 =
        unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_MULTI_THREADED, None) }?;
    if DW_FACTORY.get().is_none() {
        DW_FACTORY
            .set(unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?)
            .expect("Failed to set DW factory");
    }
    let resources = RESOURCES.get_or_init(|| match create_device_and_swap_chain() {
        Ok(res) => res,
        Err(err) => {
            panic!("Failed to create DX11 device: {}", err);
        }
    });
    unsafe {
        if D2D_CONTEXT.get().is_none() {
            match resources.device.cast::<IDXGIDevice>() {
                Ok(dxgi_device) => match d2d_factory.CreateDevice(&dxgi_device) {
                    Ok(device) => {
                        D2D_CONTEXT
                            .set(
                                device
                                    .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
                                    .unwrap(),
                            )
                            .expect("Failed to set D2D Context");
                    }
                    Err(err) => {
                        log::error!("Failed to create D2d device: {}", err);
                    }
                },
                Err(err) => {
                    log::error!("Failed to cast dxgi device: {}", err);
                }
            }
        }
    }

    Ok(())
}

pub unsafe fn create_overlay(
    sync_interval: u32,
    flags: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(resources) = RESOURCES.get() {
        let swap_chain = &resources.swap_chain;
        if let Some(dw_factory) = DW_FACTORY.get() {
            let bitmap_props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_UNKNOWN,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                colorContext: ManuallyDrop::new(None),
            };

            let text_format = unsafe {
                dw_factory.CreateTextFormat(
                    PCWSTR(wide("Consolas").as_ptr()),
                    None,
                    DWRITE_FONT_WEIGHT_REGULAR,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    24.0,
                    PCWSTR(wide("en-us").as_ptr()),
                )
            }?;
            let backbuffer: IDXGISurface = unsafe { swap_chain.GetBuffer::<IDXGISurface>(0) }?;

            if let Some(d2d_context) = D2D_CONTEXT.get() {
                let d2d_target = unsafe {
                    d2d_context.CreateBitmapFromDxgiSurface(&backbuffer, Some(&bitmap_props))
                }?;
                unsafe {
                    d2d_context.SetTarget(&d2d_target);
                    d2d_context.BeginDraw();
                }
                let size = unsafe { d2d_context.GetSize() };
                let rect_layout = D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: size.width,
                    bottom: size.height,
                };
                unsafe {
                    d2d_context.Clear(Some(&D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }));
                }
                let brush = unsafe {
                    d2d_context.CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        },
                        None,
                    )
                }?;
                unsafe {
                    d2d_context.DrawText(
                        &*wide("Hello from D2D + DWrite"),
                        &text_format,
                        &rect_layout,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                    d2d_context.EndDraw(None, None)?;
                    // cleanup
                    d2d_context.SetTarget(None);
                }
                drop(d2d_target);
                drop(backbuffer);

                present_overlay(swap_chain, sync_interval, flags);
            }
        }
    }
    Ok(())
}

pub fn present_overlay(overlay_swap_chain: &IDXGISwapChain, sync_interval: u32, flags: u32) {
    unsafe {
        IN_OVERLAY_PRESENT.store(true, Ordering::Relaxed);
        if overlay_swap_chain
            .Present(sync_interval, DXGI_PRESENT(flags))
            .is_err()
        {
            log::error!("swapchain present error: {:?}", GetLastError());
        }
        IN_OVERLAY_PRESENT.store(false, Ordering::Relaxed);
    }
}

pub(crate) unsafe fn present_hook(
    orig_swap_chain: IDXGISwapChain,
    sync_interval: u32,
    flags: u32,
) -> i32 {
    if dx11_hooks::GAME_HWND.get().is_none() {
        if let Ok(desc) = unsafe { orig_swap_chain.GetDesc() } {
            if let Err(..) = dx11_hooks::GAME_HWND.set(dx11_hooks::SafeHwnd(desc.OutputWindow)) {
                log::error!("Failed to set Game HWND")
            }
        }
    }
    if let Err(e) = setup_direct_write_devices() {
        log::error!("Failed to setup direct write devices: {}", e);
    }

    if !IN_OVERLAY_PRESENT.load(Ordering::SeqCst) {
        if let Err(err) = unsafe { create_overlay(sync_interval, flags) } {
            log::debug!("Failed to do the thing: {}", err);
        }
    }
    let res = unsafe {
        dx11_hooks::ORIGINAL_PRESENT.get().unwrap()(orig_swap_chain, sync_interval, flags)
    };
    res
}
