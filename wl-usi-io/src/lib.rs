#![no_std]
#![feature(box_vec_non_null)]
use lilium_sys::{sys::io::IOWrite, uuid::parse_uuid};
use wl_impl::{
    erase,
    helpers::insert_elems,
    syscall_handler::{SubsysInfo, register_subsys},
    syscall_helpers::SysCallTyErased,
    wl_init_subsystem_name,
};

extern crate alloc;

mod hdl_impl;

static SYSCALLS: [Option<SysCallTyErased>; 4096] =
    insert_elems([None; 4096], [(1, erase!(IOWrite))]);

static INFO: SubsysInfo = SubsysInfo {
    name: "io",
    uuid: parse_uuid("144e7137-9e85-5b9e-8d4a-8c700fb9d6bd"),
    subsys_version: 0,
    max_sysno: 128,
};

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(2, &SYSCALLS, &INFO);
    }
}

mod basic;

mod poll;

mod dev;
