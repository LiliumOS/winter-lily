#![feature(never_type)]
#![no_std]
use lilium_sys::uuid::parse_uuid;
use wl_impl::{
    erase,
    helpers::insert_elems,
    syscall_handler::{SubsysInfo, register_subsys},
    syscall_helpers::SysCallTyErased,
    wl_init_subsystem_name,
};

static SYSCALLS: [Option<SysCallTyErased>; 4096] = insert_elems([None; 4096], []);

static INFO: SubsysInfo = SubsysInfo {
    name: "thread",
    uuid: parse_uuid("f8ee4381-7db2-5c4b-bdd9-dad7f83412a4"),
    subsys_version: 0,
    max_sysno: 128,
};

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(1, &SYSCALLS, &INFO);
    }
}

mod exit;
