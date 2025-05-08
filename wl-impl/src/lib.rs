#![feature(
    thread_local,
    string_from_utf8_lossy_owned,
    naked_functions,
    never_type,
    nonzero_ops,
    exact_size_is_empty,
    iter_next_chunk,
    array_into_iter_constructors,
    iter_advance_by,
    allocator_api,
    alloc_layout_extra,
    inline_const_pat,
    once_cell_try
)]
#![no_std]

extern crate alloc;

use core::sync::atomic::AtomicI8;

pub mod eh;
pub mod env;
pub mod handle_base;
pub mod helpers;
pub mod syscall_helpers;

pub mod catch_signals;

pub mod syscall_handler;

pub mod global;

pub mod ministd;

pub mod libc;

pub mod rand;

pub mod thread;

#[cfg(not(target_os = "linux"))]
compile_error!("We only support linux for now");

use helpers::__install_sa_handler;
use ministd::Mutex;
use rand::GLOBAL_SEED;
use wl_helpers::rand::Gen;
pub use wl_interface_map::*;

#[thread_local]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
static SYS_INTERCEPT_STOP: AtomicI8 = AtomicI8::new(1);

/// Initializes the process for winter-lily
#[unsafe(export_name = wl_setup_process_name!())]
#[allow(improper_ctypes_definitions)] // We're fine here, just calling Rust-Rust
unsafe extern "C" fn __wl_impl_setup_process(
    wl_load_base: *mut u8,
    wl_load_size: usize,
    mode: FilterMode,
    rand_init: [u8; 16],
) {
    println!(
        "wl_impl_setup_process called. Protected Region ({wl_load_base:p}..{:p})",
        wl_load_base.wrapping_add(wl_load_size)
    );
    unsafe {
        __install_sa_handler();
    }
    let _ = GLOBAL_SEED.set(Mutex::new(Gen::seed(rand_init)));
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
        _ => todo!(),
    }
}

// Statically check that `wl_impl_setup_process` has the right type
const _: SetupProcessTy = __wl_impl_setup_process;

pub const LILIUM_TARGET: &str = core::env!("WL_LILIUM_TARGET");

pub mod consts {
    use crate::helpers::const_parse_u32;

    pub const ARCH: &str = core::env!("WL_LILIUM_TARGET_ARCH");
    pub const OS_NAME: &str = core::env!("WL_LILIUM_TARGET_OS");
    pub const ENV: &str = core::env!("WL_LILIUM_TARGET_ENV");

    pub const VERSION: &str = git_version::git_version!(
        prefix = ::core::concat!(::core::env!("CARGO_PKG_VERSION"), "-"),
        cargo_suffix = "-packaged"
    );

    pub const KVENDOR_NAME: &str = core::env!("WL_VENDOR_NAME");

    pub const VERSION_MAJOR: u32 = const_parse_u32(core::env!("CARGO_PKG_VERSION_MAJOR"), 10);
    pub const VERSION_MINOR: u32 = const_parse_u32(core::env!("CARGO_PKG_VERSION_MINOR"), 10);
    pub const VERSION_PATCH: u32 = const_parse_u32(core::env!("CARGO_PKG_VERSION_PATCH"), 10);

    pub const KAPI_VERSION: u64 =
        ((VERSION_MAJOR as u64) << 40) | ((VERSION_MINOR as u64) << 20) | (VERSION_PATCH as u64);
}

mod panic;
