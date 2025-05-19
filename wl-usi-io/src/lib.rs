#![no_std]
#![feature(box_vec_non_null)]
use lilium_sys::sys::io::IOWrite;
use wl_impl::{
    erase, helpers::insert_elems, syscall_handler::register_subsys,
    syscall_helpers::SysCallTyErased, wl_init_subsystem_name,
};

extern crate alloc;

mod hdl_impl;

static SYSCALLS: [Option<SysCallTyErased>; 4096] =
    insert_elems([None; 4096], [(1, erase!(IOWrite))]);

#[unsafe(export_name = wl_init_subsystem_name!())]
unsafe extern "C" fn init_subsystem() {
    unsafe {
        register_subsys(2, &SYSCALLS);
    }
}

mod basic;

mod poll;

mod dev;
