use core::{arch::global_asm, ffi::c_void};


#[cfg(target_arch = "x86_64")]
global_asm!{
    ".global __tls_get_addr",
    ".protected __tls_get_addr",
    "__tls_get_addr:",
    ""
}

