use core::arch::naked_asm;
use core::{arch::global_asm, ffi::c_void};

use lilium_sys::sys::kstr::KStrPtr;

use crate::entry::WL_RESOLVER;

#[repr(C)]
pub struct TlsDesc {
    module: usize,
    offset: usize,
}

#[cfg(target_arch = "x86_64")]
#[naked]
#[unsafe(no_mangle)]
unsafe extern "C" fn __tls_get_addr(desc: *const TlsDesc) -> *mut c_void {
    unsafe {
        naked_asm! {
            "mov rax, qword ptr [rdi]",
            "add rax, qword ptr [rdi+8]",
            "add rax, qword ptr fs:[0]",
            "ret",
            ".protected __tls_get_addr"
        }
    }
}
