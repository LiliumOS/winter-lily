use core::{
    arch::global_asm,
    ffi::{CStr, c_char},
    ptr::NonNull,
};

use ld_so_impl::helpers::cstr_from_ptr;

use crate::helpers::{FusedUnsafeCell, NullTerm, SplitAscii, SyncPointer, debug};

#[unsafe(no_mangle)]
pub static __environ: FusedUnsafeCell<SyncPointer<*mut *mut c_char>> =
    FusedUnsafeCell::new(SyncPointer::null_mut());

global_asm! {
    ".protected __environ",
}

pub fn get_env(var: &str) -> Option<&str> {
    debug("get_env", var.as_bytes());
    for ptr in
        unsafe { NullTerm::<*mut c_char>::from_ptr_unchecked(NonNull::new(__environ.0)?).copied() }
    {
        let envst =
            unsafe { NullTerm::<u8>::from_ptr_unchecked(NonNull::new_unchecked(ptr.cast())) };

        let st = envst.as_utf8().ok()?;
        let (key, val) = SplitAscii::new(st, b'=').split_once();

        if key == var {
            debug("get_env(return)", val.as_bytes());
            return Some(val);
        }
    }
    None
}

pub fn get_cenv(var: &str) -> Option<&CStr> {
    debug("get_env", var.as_bytes());
    for ptr in
        unsafe { NullTerm::<*mut c_char>::from_ptr_unchecked(NonNull::new(__environ.0)?).copied() }
    {
        let envst =
            unsafe { NullTerm::<u8>::from_ptr_unchecked(NonNull::new_unchecked(ptr.cast())) };

        let bytes = envst.as_slice();
        let st = core::str::from_utf8(bytes).ok()?;

        let (key, val) = SplitAscii::new(st, b'=').split_once();

        let pos = key.len() + 1;

        if key == var {
            debug("get_env(return)", val.as_bytes());
            return Some(unsafe { cstr_from_ptr(bytes[pos..].as_ptr().cast()) });
        }
    }
    None
}
