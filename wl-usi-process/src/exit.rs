use wl_impl::{export_syscall, libc::exit_group};

export_syscall! {
    unsafe extern fn ExitProcess(code: i32) -> ! {
        unsafe { exit_group(code).unwrap() }
    }
}
