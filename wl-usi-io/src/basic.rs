use core::ffi::c_void;

use lilium_sys::{
    result::Result,
    sys::{
        handle::{HANDLE_TYPE_IO, HandlePtr},
        io as sys,
    },
};
use wl_impl::{
    export_syscall, handle_base::Handle, helpers::linux_error_to_lilium, libc::write,
    ministd::AsRawFd as _,
};

export_syscall! {
    unsafe extern fn IOWrite(hdl: HandlePtr<sys::IOHandle>, base: *mut c_void, len: usize) -> Result<usize> {
        let hdl = unsafe { Handle::try_deref(hdl.cast())? };
        hdl.check_type(HANDLE_TYPE_IO as usize, 0xF0000000)?;
        // TODO: check capabilities
        let fd = hdl.borrow_fd().expect("Expected an IOHandle to have an attached handle");
        let v = unsafe { write(fd.as_raw_fd(), base, len) }
            .map_err(linux_error_to_lilium)?;

        Ok(v)
    }
}
