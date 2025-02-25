use core::{
    ffi::{CStr, c_char},
    ptr::NonNull,
};

use crate::helpers::{FusedUnsafeCell, NullTerm, SplitAscii, SyncPointer, debug};

pub static __ENV: FusedUnsafeCell<SyncPointer<*mut *mut c_char>> =
    FusedUnsafeCell::new(SyncPointer::null_mut());

pub fn get_env(var: &str) -> Option<&str> {
    debug("get_env", var.as_bytes());
    for ptr in
        unsafe { NullTerm::<*mut c_char>::from_ptr_unchecked(NonNull::new(__ENV.0)?).copied() }
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
    for ptr in
        unsafe { NullTerm::<*mut c_char>::from_ptr_unchecked(NonNull::new(__ENV.0)?).copied() }
    {
        let envst =
            unsafe { NullTerm::<u8>::from_ptr_unchecked(NonNull::new_unchecked(ptr.cast())) };

        let bytes = envst.as_slice();
        let st = core::str::from_utf8(bytes).ok()?;

        let (key, val) = SplitAscii::new(st, b'=').split_once();

        let pos = key.len() + 1;

        if key == var {
            return Some(unsafe { CStr::from_bytes_with_nul_unchecked(&bytes[pos..]) });
        }
    }
    None
}
