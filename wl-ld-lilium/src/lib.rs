#![no_main]
#![no_builtins]
#![no_std]
#![feature(
    slice_from_ptr_range,
    const_slice_from_ptr_range,
    try_trait_v2,
    try_trait_v2_residual,
    allocator_api,
    alloc_layout_extra,
    naked_functions,
    strict_provenance_atomic_ptr
)]

extern crate alloc;

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    crash_unrecoverably()
}

mod auxv;
use core::ffi::c_char;

use ld_so_impl::arch::crash_unrecoverably;
use ld_so_impl::elf;
mod entry;
mod helpers;
mod io;
mod ldso;
mod loader;
mod resolver;

mod env;

use ld_so_impl::{safe_addr_of, safe_addr_of_mut};
