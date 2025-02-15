use lilium_sys::result::Error;
use lilium_sys::result::Result;
use std::sync::atomic::Ordering;

unsafe extern "C" {
    pub unsafe fn __checked_memcpy_impl(dest: *mut u8, src: *const u8, len: usize) -> isize;
    pub unsafe fn __install_sa_handler();
}
