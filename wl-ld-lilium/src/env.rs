use std::{ffi::c_char, ptr::NonNull};

use crate::helpers::{FusedUnsafeCell, NullTerm, SyncPointer};

pub static __ENV: FusedUnsafeCell<SyncPointer<*mut *mut c_char>> =
    FusedUnsafeCell::new(SyncPointer::null_mut());

pub fn get_env(var: &str) -> Option<&str> {
    for ptr in
        unsafe { NullTerm::<*mut c_char>::from_ptr_unchecked(NonNull::new(__ENV.0)?).copied() }
    {
        let envst =
            unsafe { NullTerm::<u8>::from_ptr_unchecked(NonNull::new_unchecked(ptr.cast())) };

        let st = envst.as_utf8().ok()?;

        let (key, val) = st.split_once('=')?;

        if key == var {
            return Some(val);
        }
    }
    None
}
