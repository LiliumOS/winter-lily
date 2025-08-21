use core::cell::SyncUnsafeCell;

use lilium_sys::{
    result::Error,
    sys::{
        info::{self as sys, SysInfoRequest, SysInfoRequestSupportedSubsystem},
        kstr::KSlice,
    },
    uuid::{Uuid, parse_uuid},
};
use wl_impl::{
    consts, export_syscall,
    helpers::*,
    libc::{new_utsname, uname},
    syscall_handler::all_subsystems,
};

use lilium_sys::result::Result;
use wl_helpers::LazyLock;

const BASE: uuid::Uuid = uuid::uuid!("bf5db4a1-fd7d-5785-822a-d18ea774625d");

static VERSION: LazyLock<Uuid> = LazyLock::new(|| {
    let uuid = uuid::Uuid::new_v5(&BASE, consts::VERSION.as_bytes());

    uuid.into()
});

static UNAME_BUF: SyncUnsafeCell<new_utsname> = SyncUnsafeCell::new(unsafe { core::mem::zeroed() });

static COMPUTER_NAME: LazyLock<(&'static str, Uuid)> = LazyLock::new(|| {
    let str = (unsafe { uname(UNAME_BUF.get()) })
        .map_err(|_| ())
        .and_then(|_| {
            let b = unsafe { &(*UNAME_BUF.get()).nodename };
            let n = b.iter().take_while(|c| (**c) != 0).count();
            str::from_utf8(bytemuck::cast_slice(&b[..n])).map_err(|_| ())
        })
        .unwrap_or("**UNKNOWN SYSTEM NAME**!");

    let uuid = uuid::Uuid::new_v5(&BASE, str.as_bytes());

    (str, uuid.into())
});

static ARCH_TYPE_VERSION: LazyLock<(Uuid, u32)> = LazyLock::new(|| {
    cfg_match::cfg_match! {
        target_arch = "x86_64"  => ({
            let march_ver = if test_x86_features!("cmpxchg16b", "lahf_lm", "popcnt", "sse3", "sse4.1", "sse4.2", "ssse3") {
                if test_x86_features!("avx", "avx2", "bmi1", "bmi2", "f16c", "fma", "abm", "movbe", "osxsave") {
                    if test_x86_features!("avx512f", "avx512bw", "avx512cd", "avx512dq", "avx512vl") {
                        4
                    } else {
                        3
                    }
                } else {
                    2
                }
            } else {
                1
            };

            (sys::arch_info::ARCH_TYPE_X86_64, march_ver)
        }),
        _ => ({
            core::compile_error!("Unexpected Target")
        })
    }
});

fn process_request(req: &mut sys::SysInfoRequest) -> Result<()> {
    let head = unsafe { &req.head };
    // Yes, I intend to do `and` here. Not `and_then`.
    validate_option_head(head, 0x0001)?;

    if (head.flags & 0x00010000) != 0 {
        return if (head.flags & 0x0001) == 0 {
            Err(Error::InvalidOption)
        } else {
            Ok(())
        };
    }

    match head.ty {
        sys::SYSINFO_REQUEST_OSVER => {
            let req = unsafe { &mut req.os_version };
            req.head.flags &= !0x0001;
            unsafe { fill_str(&mut req.osvendor_name, consts::OS_ELAB_NAME)? };
            req.os_major = consts::OS_VERSION_MAJOR;
            req.os_minor = consts::OS_VERSION_MINOR;
            Ok(())
        }
        sys::SYSINFO_REQUEST_KVENDOR => {
            let req = unsafe { &mut req.kernel_vendor };
            req.head.flags &= !0x0001;
            unsafe { fill_str(&mut req.kvendor_name, consts::KVENDOR_NAME)? };
            req.build_id = *VERSION;
            req.kernel_major = consts::VERSION_MAJOR;
            req.kernel_minor = consts::VERSION_MINOR;
            Ok(())
        }
        sys::SYSINFO_REQUEST_ARCH_INFO => {
            let req = unsafe { &mut req.arch_info };
            req.head.flags &= !0x0001;
            let (arch_type, arch_version) = *ARCH_TYPE_VERSION;
            req.arch_type = arch_type;
            req.arch_version = arch_version;
            Ok(())
        }
        sys::SYSINFO_REQUEST_COMPUTER_NAME => {
            let req = unsafe { &mut req.computer_name };
            req.head.flags &= !0x0001;
            req.sys_id = COMPUTER_NAME.1;
            let mut ret = Ok(());
            ret = unsafe { fill_str(&mut req.hostname, COMPUTER_NAME.0).and(ret) };
            ret = unsafe { fill_str(&mut req.sys_display_name, COMPUTER_NAME.0).and(ret) };
            ret = unsafe { fill_str(&mut req.sys_label, COMPUTER_NAME.0).and(ret) };
            ret
        }
        id => {
            if let Some((subsys, info)) = all_subsystems().find(|(_, subsys)| subsys.uuid == id) {
                let req = unsafe {
                    &mut *(req as *mut SysInfoRequest as *mut SysInfoRequestSupportedSubsystem)
                };
                req.max_sysno = info.max_sysno;
                req.subsys_version = info.subsys_version;
                req.subsystem_no = subsys;
                Ok(())
            } else {
                if (head.flags & 0x0001) == 0 {
                    Err(Error::InvalidOption)
                } else {
                    Ok(())
                }
            }
        }
    }
}

export_syscall! {
    unsafe extern fn GetSystemInfo(reqs: KSlice<sys::SysInfoRequest>) -> Result<()> {
        let mut res = Ok(());
        for req in unsafe { iter_mut_checked(reqs) } {
            // This is intentional. Each request is processed independently
            res = res.and(process_request(req?));
        }

        res
    }
}
