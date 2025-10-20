use crate::ui::overlay::{present_hook, resize_hook};
use crate::utilities::DMC3_ADDRESS;
use std::ffi::c_void;
use std::fmt::Debug;
use std::ptr;
use std::sync::OnceLock;
use windows::core::HRESULT;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE, D3D_FEATURE_LEVEL};
use windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_FLAG;
use windows::Win32::Graphics::Dxgi::{
    Common, IDXGISwapChain, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_CHAIN_FLAG,
};
use windows::Win32::System::Memory::{
    VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
};

type D3D11CreateDeviceAndSwapChain = unsafe extern "system" fn(
    padapter: *mut c_void,
    drivertype: D3D_DRIVER_TYPE,
    software: HMODULE,
    flags: D3D11_CREATE_DEVICE_FLAG,
    pfeaturelevels: *const D3D_FEATURE_LEVEL,
    featurelevels: u32,
    sdkversion: u32,
    pswapchaindesc: *const DXGI_SWAP_CHAIN_DESC,
    ppswapchain: *mut *mut IDXGISwapChain,
    ppdevice: *mut *mut c_void,
    pfeaturelevel: *mut D3D_FEATURE_LEVEL,
    ppimmediatecontext: *mut *mut c_void,
) -> HRESULT;

type PresentFn = unsafe extern "system" fn(IDXGISwapChain, u32, u32) -> i32; // *mut IDXGISwapChain
type ResizeBuffersFn = unsafe extern "system" fn(
    *mut IDXGISwapChain,
    u32,
    u32,
    u32,
    Common::DXGI_FORMAT,
    DXGI_SWAP_CHAIN_FLAG,
);

static ORIGINAL_DEV_CHAIN: OnceLock<D3D11CreateDeviceAndSwapChain> = OnceLock::new();
pub(crate) static ORIGINAL_PRESENT: OnceLock<PresentFn> = OnceLock::new();
pub(crate) static ORIGINAL_RESIZE_BUFFERS: OnceLock<ResizeBuffersFn> = OnceLock::new();

unsafe extern "system" fn hook_d3d11_create_device_and_swap_chain(
    padapter: *mut c_void,
    drivertype: D3D_DRIVER_TYPE,
    software: HMODULE,
    flags: D3D11_CREATE_DEVICE_FLAG,
    pfeaturelevels: *const D3D_FEATURE_LEVEL,
    featurelevels: u32,
    sdkversion: u32,
    pswapchaindesc: *const DXGI_SWAP_CHAIN_DESC,
    ppswapchain: *mut *mut IDXGISwapChain,
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
    install_present_hook(ppswapchain);
    log::debug!("Installed present hook");
    install_resize_hook(ppswapchain);
    log::debug!("Installed resize hook");
    res
}

// TODO Consolidate these two and make them return Result's
fn install_present_hook(ppswapchain: *mut *mut IDXGISwapChain) {
    unsafe {
        if ppswapchain.is_null() {
            return;
        }
        let swap_ptr = *ppswapchain;
        if swap_ptr.is_null() {
            return;
        }
        install(
            *(swap_ptr as *const *const usize).add(8) as *mut PresentFn,
            present_hook,
            &ORIGINAL_PRESENT,
        );
    }
}

fn install_resize_hook(ppswapchain: *mut *mut IDXGISwapChain) {
    unsafe {
        if ppswapchain.is_null() {
            return;
        }
        let swap_ptr = *ppswapchain;
        if swap_ptr.is_null() {
            return;
        }
        install(
            *(swap_ptr as *const *const usize).add(13) as *mut ResizeBuffersFn,
            resize_hook,
            &ORIGINAL_RESIZE_BUFFERS,
        );
    }
}

pub fn setup_overlay() {
    log::info!("Setting up Archipelago Randomizer Overlay");
    install(
        (*DMC3_ADDRESS + 0x34F650) as *mut D3D11CreateDeviceAndSwapChain,
        hook_d3d11_create_device_and_swap_chain,
        &ORIGINAL_DEV_CHAIN,
    );
    log::debug!("Installed device and swap chain hook");
}

fn install<T>(dest: *mut T, hook: T, original: &OnceLock<T>)
where
    T: Copy + 'static + Debug,
{
    unsafe {
        let orig = ptr::read(dest);
        let size = size_of::<T>();
        let mut old_protect = PAGE_PROTECTION_FLAGS::default();
        let addr = dest as *mut _;
        if VirtualProtect(addr, size, PAGE_EXECUTE_READWRITE, &mut old_protect).is_err() {
            log::error!("Install VirtualProtect (1) failed");
        }
        ptr::write(dest, hook.into());
        if VirtualProtect(addr, size, old_protect, &mut old_protect).is_err() {
            log::error!("Install VirtualProtect (2) failed");
        }
        if let Err(err) = original.set(orig) {
            log::error!("Failed to install overlay related hook: {:?}", err);
        }
    }
}
