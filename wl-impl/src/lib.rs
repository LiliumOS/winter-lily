#![feature(
    thread_local,
    string_from_utf8_lossy_owned,
    naked_functions,
    never_type
)]
use core::sync::atomic::AtomicI8;

pub mod env;
pub mod handle_base;
pub mod helpers;
pub mod syscall_helpers;

pub mod catch_signals;

pub mod syscall_handler;

#[cfg(not(target_os = "linux"))]
compile_error!("We only support linux for now");

#[non_exhaustive]
pub enum FilterMode {
    /// Use prctl to emulate syscalls
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Prctl,
    /// Use `seccomp`, instead of prctl.
    /// Note that this does not imply full permission enforcement - only Linux level permissions are used by default.
    ///
    Seccomp,
}

#[thread_local]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
static SYS_INTERCEPT_STOP: AtomicI8 = AtomicI8::new(1);

/// Initializes the process for winter-lily
pub unsafe fn setup_process(wl_load_base: *mut u8, wl_load_size: usize, mode: FilterMode) {
    match mode {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        FilterMode::Prctl => {
            if unsafe {
                linux_syscall::syscall!(
                    linux_syscall::SYS_prctl,
                    59,
                    1,
                    wl_load_base,
                    wl_load_size,
                    SYS_INTERCEPT_STOP.as_ptr(),
                )
                .as_usize_unchecked() as isize
            } < 0
            {
                todo!()
            }
        }
        FilterMode::Seccomp => todo!(),
    }
}

pub const LILIUM_TARGET: &str = core::env!("WL_LILIUM_TARGET");
