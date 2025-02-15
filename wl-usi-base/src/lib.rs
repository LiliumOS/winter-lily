#![feature(never_type)]
use lilium_sys::sys::sysno::base::SYS_UnmanagedException;
use wl_impl::{
    erase, helpers::insert_elems, syscall_handler::register_subsys,
    syscall_helpers::SysCallTyErased,
};

pub mod except;
pub mod info;

static SYSCALLS: [Option<SysCallTyErased>; 4096] = insert_elems([None; 4096], [(
    SYS_UnmanagedException,
    erase!(except::UnmanagedException),
)]);

#[unsafe(no_mangle)]
unsafe extern "C" fn __init_subsystem() {
    unsafe {
        register_subsys(0, &SYSCALLS);
    }
}
