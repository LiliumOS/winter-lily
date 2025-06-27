#![no_std]
#![feature(never_type, unwrap_infallible)]
use exit::ExitProcess;
use lilium_sys::uuid::parse_uuid;
use mem::*;
use wl_impl::{
    erase,
    helpers::insert_elems,
    syscall_handler::{SubsysInfo, register_subsys},
    syscall_helpers::SysCallTyErased,
    wl_init_subsystem_name,
};

extern crate alloc;

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

static INFO: SubsysInfo = SubsysInfo {
    name: "process",
    uuid: parse_uuid("2bf86506-9b4a-5065-ac9e-ad6d21027460"),
    subsys_version: 0,
    max_sysno: 128,
};

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(3, &SYSCALLS, &INFO);
    }
}

mod exit;
mod mem;
mod proc;
