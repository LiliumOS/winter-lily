#![no_std]
#![feature(never_type)]
use lilium_sys::uuid::parse_uuid;
use wl_impl::{
    InitSubsystemTy, erase,
    helpers::insert_elems,
    syscall_handler::{SubsysInfo, register_subsys},
    syscall_helpers::SysCallTyErased,
    wl_init_subsystem_name,
};

static SYSCALLS: [Option<SysCallTyErased>; 4096] = insert_elems([None; 4096], []);

static INFO: SubsysInfo = SubsysInfo {
    name: "kmgmt",
    uuid: parse_uuid("90bd3c96-a8e1-5896-a9f2-704e98abec9f"),
    subsys_version: 0,
    max_sysno: 128,
};

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(5, &SYSCALLS, &INFO);
    }
}
const _: InitSubsystemTy = init_subsystem;
