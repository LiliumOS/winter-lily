#![no_std]

use lilium_sys::sys::{
    handle::{Handle, HandlePtr},
    kstr::KSlice,
};

#[non_exhaustive]
#[repr(usize)]
pub enum FilterMode {
    /// Use prctl to emulate syscalls
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Prctl,
    /// Use `seccomp`, instead of prctl.
    /// Note that this does not imply full permission enforcement - only Linux level permissions are used by default.
    ///
    Seccomp,
}

pub type SetupProcessTy = unsafe extern "C" fn(
    wl_load_base: *mut u8,
    wl_load_size: usize,
    mode: FilterMode,
    rand_init: [u8; 16],
);

/// # Safety
/// Must be called at most once per module before any other code (other than DT_INIT/DT_INITARR elements) is run
pub type InitSubsystemTy = unsafe extern "C" fn();

#[macro_export]
macro_rules! wl_setup_process_name {
    () => {
        "__wl_init_setup_process_v0"
    };
    (C) => {
        c"__wl_init_setup_process_v0"
    };
}

#[macro_export]
macro_rules! wl_init_subsystem_name {
    () => {
        "__wl_init_subsystem_v0"
    };
    (C) => {
        c"__wl_init_subsystem_v0"
    };
}

pub type GetInitHandlesTy = unsafe extern "C" fn(&mut KSlice<HandlePtr<Handle>>);

#[macro_export]
macro_rules! wl_get_init_handles_name {
    () => {
        "__wl_get_init_handles_v0"
    };
    (C) => {
        c"__wl_get_init_handles_v0"
    };
}
