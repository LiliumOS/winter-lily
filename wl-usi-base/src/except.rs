use lilium_sys::sys::except::ExceptionInfo;
use wl_impl::{export_syscall, helpers::exit_unrecoverably};

export_syscall! {
    unsafe extern fn UnmanagedException(_ptr: *const ExceptionInfo) -> ! {
        // we don't yet support exception reporting, so just hard-crash.
        exit_unrecoverably()
    }
}
