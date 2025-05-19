use wl_impl::{
    export_syscall, handle_base::Handle, helpers::linux_error_to_lilium, libc::write,
    ministd::AsRawFd as _,
};

use lilium_sys::{
    result::{Error, Result},
    sys::{
        device::{self as sys, DeviceHandle},
        handle::HandlePtr,
    },
    uuid::Uuid,
};

export_syscall! {
    unsafe extern fn OpenDevice(hdl: *mut HandlePtr<DeviceHandle>, id: Uuid) -> Result<()> {

        Err(Error::UnknownDevice)
    }
}
