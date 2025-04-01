#![feature(never_type)]
#![no_std]
use lilium_sys::sys::sysno::base::SYS_UnmanagedException;
use wl_impl::{
    erase, helpers::insert_elems, syscall_handler::register_subsys,
    syscall_helpers::SysCallTyErased, wl_init_subsystem_name,
};

pub mod except;
pub mod info;

static SYSCALLS: [Option<SysCallTyErased>; 4096] = insert_elems(
    [None; 4096],
    [(SYS_UnmanagedException, erase!(except::UnmanagedException))],
);

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(0, &SYSCALLS);
    }
}
