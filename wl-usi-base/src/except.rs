use lilium_sys::sys::except::ExceptionInfo;
use wl_impl::{export_syscall, helpers::exit_unrecoverably};

export_syscall! {
    unsafe extern fn UnmanagedException(ptr: *const ExceptionInfo) -> ! {
        exit_unrecoverably()
    }
}
