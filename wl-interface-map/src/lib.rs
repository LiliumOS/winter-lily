#![no_std]

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

pub type SetupProcessTy =
    unsafe extern "C" fn(wl_load_base: *mut u8, wl_load_size: usize, mode: FilterMode);

pub type InitSubsystemTy = extern "C" fn();

#[macro_export]
macro_rules! wl_setup_process_name {
    () => {
        "__wl_init_setup_process"
    };
    (C) => {
        c"__wl_init_setup_process"
    };
}

#[macro_export]
macro_rules! wl_init_subsystem_name {
    () => {
        "__wl_init_subsystem"
    };
    (C) => {
        c"__wl_init_subsystem"
    };
}
