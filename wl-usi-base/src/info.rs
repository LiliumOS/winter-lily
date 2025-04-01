use lilium_sys::{
    result::Error,
    sys::{info as sys, kstr::KSlice},
    uuid::{Uuid, parse_uuid},
};
use wl_impl::{consts, export_syscall, helpers::*};

use lilium_sys::result::Result;
use wl_helpers::LazyLock;

const BASE: uuid::Uuid = uuid::uuid!("bf5db4a1-fd7d-5785-822a-d18ea774625d");

static VERSION: LazyLock<Uuid> = LazyLock::new(|| {
    let uuid = uuid::Uuid::new_v5(&BASE, consts::VERSION.as_bytes());

    uuid.into()
});

static ARCH_TYPE_VERSION: LazyLock<(Uuid, u32)> = LazyLock::new(|| {
    cfg_match::cfg_match! {
        target_arch = "x86_64"  => ({
            (sys::arch_info::ARCH_TYPE_X86_64, 0)
        }),
        _ => ({
            core::compile_error!("Unexpected Target")
        })
    }
});

export_syscall! {
    unsafe extern fn GetSystemInfo(reqs: KSlice<sys::SysInfoRequest>) -> Result<()> {
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
                sys::SYSINFO_REQUEST_OSVER => {
                    let req = unsafe{ &mut req.os_version };
                    req.head.flags &= !0x0001;
                    res = unsafe { fill_str(&mut req.osvendor_name, consts::KVENDOR_NAME).and(res) };
                    req.os_major = consts::VERSION_MAJOR;
                    req.os_minor = consts::VERSION_MINOR;
                }
                sys::SYSINFO_REQUEST_KVENDOR => {
                    let req = unsafe{ &mut req.kernel_vendor };
                    req.head.flags &= !0x0001;
                    res = unsafe { fill_str(&mut req.kvendor_name, consts::KVENDOR_NAME).and(res) };
                    req.build_id = *VERSION;
                    req.kernel_major = consts::VERSION_MAJOR;
                    req.kernel_minor = consts::VERSION_MINOR;
                }
                sys::SYSINFO_REQUEST_ARCH_INFO => {
                    let req = unsafe{ &mut req.arch_info };
                    req.head.flags &= !0x0001;
                    let (arch_type, arch_version) = *ARCH_TYPE_VERSION;
                    req.arch_type = arch_type;
                    req.arch_version = arch_version;
                }
                _ => res = res.and(if (head.flags & 0x0001) == 0 {Err(Error::InvalidOption)} else {Ok(())}),
            }

        }

        res
    }
}
