#![no_main]
#![feature(
    slice_from_ptr_range,
    const_slice_from_ptr_range,
    try_trait_v2,
    try_trait_v2_residual,
    allocator_api,
    alloc_layout_extra
)]

mod auxv;
use std::ffi::c_char;

use ld_so_impl::elf;
mod entry;
mod helpers;
mod io;
mod ldso;
mod resolver;

mod env;

use ld_so_impl::{safe_addr_of, safe_addr_of_mut};
