use core::ffi::c_void;

use lilium_sys::result::Result;
use lilium_sys::sys::except::ExceptionStatusInfo;
use wl_impl::{eprintln, export_syscall, helpers::exit_unrecoverably};

export_syscall! {
    unsafe extern fn UnmanagedException(ptr: *const ExceptionStatusInfo) -> ! {
        exit_unrecoverably(Some(unsafe{(*ptr).except_code}))
    }
}

export_syscall! {
    unsafe extern fn ExceptHandleSynchronous(ptr: *const ExceptionStatusInfo, _data: *const c_void) -> Result<()> {
        // TODO: There is more stuff to do, but for now, treat it as though exceptions are all unmanaged
        exit_unrecoverably(Some(unsafe{(*ptr).except_code}))
    }
}
