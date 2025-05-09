use core::arch::global_asm;

use crate::helpers::CheckedAccessError;

unsafe extern "C" {
    unsafe fn __checked_memcpy_impl(
        dest: *mut u8,
        src: *const u8,
        len: usize,
        err: &mut CheckedAccessError,
    ) -> isize;
    pub unsafe fn __install_sa_handler();
}

global_asm! {
    ".protected {__checked_memcpy_impl}",
    ".protected {__install_sa_handler}",
    __install_sa_handler = sym __install_sa_handler,
    __checked_memcpy_impl = sym __checked_memcpy_impl,
}

#[inline(never)]
pub unsafe fn checked_memcpy_impl(
    dest: *mut u8,
    src: *const u8,
    len: usize,
    err: &mut CheckedAccessError,
) -> isize {
    unsafe { __checked_memcpy_impl(dest, src, len, err) }
}
