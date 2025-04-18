use lilium_sys::sys::except::ExceptionStatusInfo;
use wl_impl::{eprintln, export_syscall, helpers::exit_unrecoverably};

export_syscall! {
    unsafe extern fn UnmanagedException(ptr: *const ExceptionStatusInfo) -> ! {
        exit_unrecoverably(Some(unsafe{(*ptr).except_code}))
    }
}
