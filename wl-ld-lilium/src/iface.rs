use core::arch::naked_asm;
use core::{arch::global_asm, ffi::c_void};

use lilium_sys::sys::kstr::KStrPtr;

use crate::entry::WL_RESOLVER;
use crate::loader::{get_tp, update_tls};

#[repr(C)]
pub struct TlsDesc {
    module: usize,
    offset: usize,
}

#[cfg(target_arch = "x86_64")]
#[naked]
#[unsafe(no_mangle)]
unsafe extern "C" fn __tls_get_addr(desc: *const TlsDesc) -> *mut c_void {
    use core::mem::offset_of;

    use crate::loader::{TLS_MC, Tcb};

    unsafe {
        naked_asm! {
            "mov rax, qword ptr [{TLS_MC}+rip]",
            "mov rax, qword ptr [rax + {load_size}]",
            "test rax, qword ptr fs:[{load_size}]",
            "ja 3f",
            "2:",
            "mov rax, qword ptr [rdi]",
            "add rax, qword ptr [rdi+8]",
            "add rax, qword ptr fs:[0]",
            "ret",
            "3:",
            "push rsi", // Stack is now 16-byte aligned

            "push rdx",
            "push rcx",
            "push r8",
            "push r9",
            "push r10",
            "push r11",
            "sub rsp, 464",
            "fxsave64 [rsp]",
            "call __rtld_update_global_tcb",
            "fxrstor64 [rsp]",
            "add rsp, 464",
            "pop r11",
            "pop r10",
            "pop r9",
            "pop r8",
            "pop rcx",
            "pop rdx",

            "pop rsi",
            ".protected __tls_get_addr",
            load_size = const offset_of!(Tcb, load_size),
            TLS_MC = sym TLS_MC
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn __rtld_get_thread_ptr() -> *mut c_void {
    update_tls();
    get_tp()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn __rtld_update_global_tcb() {
    update_tls()
}
