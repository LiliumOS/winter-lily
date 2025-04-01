#![no_std]
use wl_impl::{
    helpers::insert_elems, syscall_handler::register_subsys, syscall_helpers::SysCallTyErased,
    wl_init_subsystem_name,
};

mod hdl_impl;

static SYSCALLS: [Option<SysCallTyErased>; 4096] = insert_elems([None; 4096], []);

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(2, &SYSCALLS);
    }
}
