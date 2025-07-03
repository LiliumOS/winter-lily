#![feature(never_type, sync_unsafe_cell)]
#![no_std]
use lilium_sys::{
    sys::sysno::base::{SYS_GetSystemInfo, SYS_UnmanagedException},
    uuid::parse_uuid,
};
use wl_impl::{
    erase,
    helpers::insert_elems,
    syscall_handler::{SubsysInfo, register_subsys},
    syscall_helpers::SysCallTyErased,
    wl_init_subsystem_name,
};

pub mod except;
pub mod info;

static SYSCALLS: [Option<SysCallTyErased>; 4096] = insert_elems(
    [None; 4096],
    [
        (SYS_UnmanagedException, erase!(except::UnmanagedException)),
        (SYS_GetSystemInfo, erase!(info::GetSystemInfo)),
    ],
);

static INFO: SubsysInfo = SubsysInfo {
    name: "base",
    uuid: parse_uuid("9e780f9e-f35f-580a-ada0-444d0c24e3ea"),
    subsys_version: 0,
    max_sysno: 128,
};

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(0, &SYSCALLS, &INFO);
    }
}
