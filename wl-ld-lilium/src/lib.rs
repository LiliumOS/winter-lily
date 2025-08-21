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
    strict_provenance_atomic_ptr
)]

macro_rules! println {
    ($($tt:tt)*) => {
        {
            use ::core::fmt::Write as _;
            ::core::writeln!(&$crate::io::STDOUT, $($tt)*).expect("Write to stdout failed");
        }
    };
}

macro_rules! eprintln {
    ($($tt:tt)*) => {
        {
            use ::core::fmt::Write as _;
            ::core::writeln!(&$crate::io::STDERR, $($tt)*).expect("Write to stdout failed")
        }
    };
}

extern crate alloc;

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    if let Some(loc) = info.location() {
        eprintln!("panicked at {loc}: {}", info.message())
    } else {
        eprintln!("panicked: {}", info.message())
    }

    crash_unrecoverably()
}

mod auxv;

use ld_so_impl::arch::crash_unrecoverably;
use ld_so_impl::elf;
mod entry;
mod helpers;
mod iface;
mod io;
mod ldso;
mod loader;
mod resolver;

mod detect;
mod env;
