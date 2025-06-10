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
    name: "debug",
    uuid: parse_uuid("630603ac-38d2-5936-9083-ed87a50b3453"),
    subsys_version: 0,
    max_sysno: 128,
};

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(4, &SYSCALLS, &INFO);
    }
}
const _: InitSubsystemTy = init_subsystem;
