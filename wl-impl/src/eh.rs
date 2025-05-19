use core::cell::Cell;

use crate::libc::{mcontext_t, ucontext_t};

pub struct ExceptionContext {
    unix_context: mcontext_t,
    fsave: [u64; 64],
}

pub use lilium_sys::sys::except::ExceptionInfo;
use lilium_sys::sys::except::ExceptionStatusInfo;

#[thread_local]
static EINFO_CURRENT: Cell<*mut ExceptionInfo> = Cell::new(core::ptr::null_mut());
