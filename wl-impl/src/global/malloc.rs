use core::{
    alloc::GlobalAlloc,
    ffi::c_void,
    sync::atomic::{AtomicPtr, AtomicUsize},
};

use linux_raw_sys::general::{MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};
use pooled_arena_malloc::pooled_alloc::PooledAlloc;
use wl_helpers::{MmapAllocator, OnceLock};

#[global_allocator]
static ALLOC: PooledAlloc<MmapAllocator> =
    PooledAlloc::new(MmapAllocator::new_with_hint(core::ptr::null_mut()));
