#![no_std]
#![feature(never_type)]
use exit::ExitProcess;
use wl_impl::{
    erase, helpers::insert_elems, syscall_handler::register_subsys,
    syscall_helpers::SysCallTyErased, wl_init_subsystem_name,
};

static SYSCALLS: [Option<SysCallTyErased>; 4096] =
    insert_elems([None; 4096], [(0, erase!(ExitProcess))]);

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(3, &SYSCALLS);
    }
}

mod exit;
