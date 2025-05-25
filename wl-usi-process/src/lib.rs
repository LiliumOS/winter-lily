#![no_std]
#![feature(never_type)]
use exit::ExitProcess;
use mem::*;
use wl_impl::{
    erase, helpers::insert_elems, syscall_handler::register_subsys,
    syscall_helpers::SysCallTyErased, wl_init_subsystem_name,
};

static SYSCALLS: [Option<SysCallTyErased>; 4096] = insert_elems(
    [None; 4096],
    [
        (0, erase!(ExitProcess)),
        (0x30, erase!(CreateMapping)),
        (0x31, erase!(ChangeMappingAttributes)),
        (0x32, erase!(RemoveMapping)),
        (0x33, erase!(ResizeMapping)),
    ],
);

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(3, &SYSCALLS);
    }
}

mod exit;
mod mem;
