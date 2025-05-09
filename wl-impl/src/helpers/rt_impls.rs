use core::arch::global_asm;

use crate::helpers::CheckedAccessError;

#[link(name = "signal_support", kind = "static")]
unsafe extern "C" {
    pub unsafe fn __checked_memcpy_impl(
        dest: *mut u8,
        src: *const u8,
        len: usize,
        err: &mut CheckedAccessError,
    ) -> isize;
    pub(crate) unsafe fn __install_sa_handler();
}

global_asm! {
    ".protected {__checked_memcpy_impl}",
    ".protected {__install_sa_handler}",
    __install_sa_handler = sym __install_sa_handler,
    __checked_memcpy_impl = sym __checked_memcpy_impl,
}
