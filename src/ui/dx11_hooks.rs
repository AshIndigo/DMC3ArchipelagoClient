use crate::ui::overlay::{present_hook, resize_hook};
use crate::utilities::DMC3_ADDRESS;
use randomizer_utilities::ui::dx11::{
    D3D11CreateDeviceAndSwapChain, ORIGINAL_DEV_CHAIN, ORIGINAL_PRESENT, ORIGINAL_RESIZE_BUFFERS,
    PresentFn, ResizeBuffersFn,
};
use std::error::Error;
use std::ffi::c_void;
use std::fmt::Debug;
use std::ptr;
use std::sync::OnceLock;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE, D3D_FEATURE_LEVEL};
use windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_FLAG;
use windows::Win32::Graphics::Dxgi::{DXGI_SWAP_CHAIN_DESC, IDXGISwapChain};
use windows::core::HRESULT;

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
    match install_vtable_hook(ppswapchain, 8, present_hook as PresentFn, &ORIGINAL_PRESENT) {
        Ok(_) => {
            log::debug!("Installed present hook");
        }
        Err(err) => {
            log::error!("Failed to install present hook: {}", err);
        }
    }

    match install_vtable_hook(
        ppswapchain,
        13,
        resize_hook as ResizeBuffersFn,
        &ORIGINAL_RESIZE_BUFFERS,
    ) {
        Ok(_) => {
            log::debug!("Installed resize hook");
        }
        Err(err) => {
            log::error!("Failed to install resize hook: {}", err);
        }
    }

    res
}

fn install_vtable_hook<T>(
    ppswapchain: *mut *mut IDXGISwapChain,
    vtable_idx: usize,
    hook: T,
    original: &OnceLock<T>,
) -> Result<(), Box<dyn Error>>
where
    T: Copy + 'static + Debug,
{
    unsafe {
        if ppswapchain.is_null() {
            return Err("ppswapchain was null".into());
        }
        let swap_ptr = *ppswapchain;
        if swap_ptr.is_null() {
            return Err("swap_ptr was null".into());
        }
        let vtable = *(swap_ptr as *const *const usize);
        install(vtable.add(vtable_idx) as *mut T, hook, original);
    }
    Ok(())
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
    let orig = unsafe { ptr::read(dest) };
    randomizer_utilities::modify_protected_memory(
        || unsafe {
            ptr::write(dest, hook);
        },
        dest,
    )
    .unwrap();
    if let Err(err) = original.set(orig) {
        log::error!("Failed to install overlay related hook: {:?}", err);
    }
}
