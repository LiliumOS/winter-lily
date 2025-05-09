use wl_impl::{export_syscall, libc::exit};

export_syscall! {
    unsafe extern fn ExitThread(code: i32) -> ! {
        unsafe { exit(code).unwrap() }
    }
}
