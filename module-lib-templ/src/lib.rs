#![no_std]
#![feature(never_type)]
use wl_impl::{
    erase, helpers::insert_elems, syscall_handler::register_subsys,
    syscall_helpers::SysCallTyErased, wl_init_subsystem_name, InitSubsystemTy,
};

static SYSCALLS: [Option<SysCallTyErased>; 4096] = insert_elems([None; 4096], []);

static INFO: SubsysInfo = SubsysInfo {
    name: todo!("<Name of subsys here>"),
    uuid: parse_uuid(todo!("<UUID of Subsys here>")),
    subsys_version: 0,
    max_sysno: 128,
};

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(!0, &SYSCALLS, &INFO);
    }
}

const _: InitSubsystemTy = init_subsystem;
