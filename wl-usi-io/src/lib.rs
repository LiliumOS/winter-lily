use wl_impl::{
    helpers::insert_elems, syscall_handler::register_subsys, syscall_helpers::SysCallTyErased,
};

mod hdl_impl;

static SYSCALLS: [Option<SysCallTyErased>; 4096] = insert_elems([None; 4096], []);

#[unsafe(no_mangle)]
unsafe extern "C" fn __init_subsystem() {
    unsafe {
        register_subsys(2, &SYSCALLS);
    }
}
