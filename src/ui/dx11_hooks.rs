use minhook::{MinHook, MH_STATUS};
use std::ffi::c_void;
use std::ptr;
use std::sync::OnceLock;
use windows::core::BOOL;
use windows::core::HRESULT;
use windows::core::PCSTR;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory, IDXGIAdapter, IDXGIFactory, IDXGISwapChain, DXGI_MWA_NO_ALT_ENTER, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_EFFECT_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_DESC, DXGI_SAMPLE_DESC};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

type D3D11CreateDeviceAndSwapChain = unsafe extern "system" fn(
    padapter: *mut c_void,
    drivertype: D3D_DRIVER_TYPE,
    software: HMODULE,
    flags: D3D11_CREATE_DEVICE_FLAG,
    pfeaturelevels: *const D3D_FEATURE_LEVEL,
    featurelevels: u32,
    sdkversion: u32,
    pswapchaindesc: *const DXGI_SWAP_CHAIN_DESC,
    ppswapchain: *mut *mut c_void,
    ppdevice: *mut *mut c_void,
    pfeaturelevel: *mut D3D_FEATURE_LEVEL,
    ppimmediatecontext: *mut *mut c_void,
) -> HRESULT;

type PresentFn = unsafe extern "system" fn(IDXGISwapChain, u32, u32) -> i32; // *mut IDXGISwapChain

static ORIGINAL_DEV_CHAIN: OnceLock<D3D11CreateDeviceAndSwapChain> = OnceLock::new();

static PRESENT_PTR: OnceLock<usize> = OnceLock::new();
pub(crate) static ORIGINAL_PRESENT: OnceLock<PresentFn> = OnceLock::new();

#[derive(Copy, Clone, Debug)]
pub(crate) struct SafeHwnd(pub HWND);

unsafe impl Send for SafeHwnd {}
unsafe impl Sync for SafeHwnd {}

pub(crate) static GAME_HWND: OnceLock<SafeHwnd> = OnceLock::new();

pub struct Resources {
    pub(crate) device: ID3D11Device,
    pub(crate) _device_context: ID3D11DeviceContext,
    pub(crate) swap_chain: IDXGISwapChain,
}


pub fn wide(s: &str) -> Vec<u16> {
    use std::iter::once;
    s.encode_utf16().chain(once(0)).collect()
}

fn get_cdasc() -> Option<usize> {
    match unsafe { GetModuleHandleW(PCWSTR::from_raw(wide("d3d11.dll\0").as_ptr())) } {
        Ok(hmodule) => {
            match unsafe {
                GetProcAddress(
                    hmodule,
                    PCSTR::from_raw(b"D3D11CreateDeviceAndSwapChain\0".as_ptr()),
                )
            } {
                None => {
                    log::error!("CDASC address was None");
                    None
                }
                Some(addr) => Some(addr as usize),
            }
        }
        Err(err) => {
            log::error!("Error getting d3d11 module handle: {:?}", err);
            None
        }
    }
}

unsafe fn hook_d3d11_create_device_and_swap_chain(
    padapter: *mut c_void,
    drivertype: D3D_DRIVER_TYPE,
    software: HMODULE,
    flags: D3D11_CREATE_DEVICE_FLAG,
    pfeaturelevels: *const D3D_FEATURE_LEVEL,
    featurelevels: u32,
    sdkversion: u32,
    pswapchaindesc: *const DXGI_SWAP_CHAIN_DESC,
    ppswapchain: *mut *mut c_void,
    ppdevice: *mut *mut c_void,
    pfeaturelevel: *mut D3D_FEATURE_LEVEL,
    ppimmediatecontext: *mut *mut c_void,
) -> HRESULT {
    let res = unsafe {
        ORIGINAL_DEV_CHAIN.get().unwrap()(
            padapter,
            drivertype,
            software,
            flags,
            pfeaturelevels,
            featurelevels,
            sdkversion,
            pswapchaindesc,
            ppswapchain,
            ppdevice,
            pfeaturelevel,
            ppimmediatecontext,
        )
    };

    if PRESENT_PTR.get().is_none() {
        let swap_chain_ptr = unsafe { *ppswapchain };
        if !swap_chain_ptr.is_null() {
            let vtable = unsafe { *(swap_chain_ptr as *const *const *const c_void) };
            if !vtable.is_null() {
                let present_ptr = unsafe { *vtable.add(8) }; // Present is slot 8
                PRESENT_PTR.set(present_ptr as usize).unwrap();
                unsafe {
                    ORIGINAL_PRESENT
                        .set(std::mem::transmute::<_, PresentFn>(
                            MinHook::create_hook(
                                present_ptr as *mut _,
                                crate::ui::overlay::present_hook as _,
                            )
                            .expect("Failed to create hook"),
                        ))
                        .expect("Failed to set original device chain");
                    MinHook::enable_hook(present_ptr as _).expect("Failed to enable present hook");
                }
            }
        }
    }

    res
}

pub fn install_hook() -> Result<(), MH_STATUS> {
    if let Some(create_swap_and_device_addr) = get_cdasc() {
        unsafe {
            if ORIGINAL_DEV_CHAIN.get().is_none() {
                ORIGINAL_DEV_CHAIN
                    .set(std::mem::transmute::<_, D3D11CreateDeviceAndSwapChain>(
                        MinHook::create_hook(
                            create_swap_and_device_addr as *mut _,
                            hook_d3d11_create_device_and_swap_chain as _,
                        )
                        .expect("Failed to create hook"),
                    ))
                    .expect("Failed to set original device chain");
            }

            if let Err(err) = MinHook::enable_hook(create_swap_and_device_addr as *mut _) {
                log::error!(
                    "Failed to enable {:#X} hook: {:?}",
                    create_swap_and_device_addr,
                    err
                );
            }
        }
    } else {
        log::error!("Failed to get Create Swap And Device Address");
    }

    Ok(())
}

pub fn create_device_and_swap_chain(
) -> windows::core::Result<Resources> {
    let dxgi_factory: IDXGIFactory = unsafe { CreateDXGIFactory() }?;
    let dxgi_adapter: IDXGIAdapter = unsafe { dxgi_factory.EnumAdapters(0) }?;
    let window = GAME_HWND.get().unwrap().0;
    let mut device = None;
    let mut device_context = None;
    unsafe {
        D3D11CreateDevice(
            &dxgi_adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE(ptr::null_mut()),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut device_context),
        )
    }?;
    let device = device.unwrap();
    let device_context = device_context.unwrap();

    let swap_chain_desc = DXGI_SWAP_CHAIN_DESC {
        BufferDesc: DXGI_MODE_DESC {
            Width: 0,
            Height: 0,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ..DXGI_MODE_DESC::default()
        },
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        OutputWindow: window,
        Windowed: BOOL(1),
        SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
        Flags: 0,
    };

    let mut swap_chain = None;
    unsafe { dxgi_factory.CreateSwapChain(&device, &swap_chain_desc, &mut swap_chain) }.ok()?;
    let swap_chain = swap_chain.unwrap();

    unsafe { dxgi_factory.MakeWindowAssociation(window, DXGI_MWA_NO_ALT_ENTER) }?;
    Ok(Resources{device, _device_context: device_context, swap_chain })
}
