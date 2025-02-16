use lilium_sys::{
    result::Error,
    sys::{info as sys, kstr::KSlice},
};
use wl_impl::{export_syscall, helpers::*};

use lilium_sys::result::Result;

export_syscall! {
    unsafe extern fn GetSystemInfo(reqs: KSlice<sys::SysInfoRequest>) -> Result<()> {
        let mut reqs = reqs;
        let mut res = Ok(());
        for req in unsafe { iter_mut_checked(reqs) } {
            let req = req?;
            let head = unsafe { &req.head };
            // Yes, I intend to do `and` here. Not `and_then`.
            res = res.and(validate_option_head(head, 0x0001));

            if (head.flags & 0x00010000) != 0{
                res = res.and(if (head.flags & 0x0001) == 0 {Err(Error::InvalidOption)} else {Ok(())});
                continue
            }

            match head.ty {
                sys::SYSINFO_REQUEST_KVENDOR => {
                    let req = unsafe{ &req.kernel_vendor };
                }
                _ => res = res.and(if (head.flags & 0x0001) == 0 {Err(Error::InvalidOption)} else {Ok(())}),
            }

        }

        res
    }
}
