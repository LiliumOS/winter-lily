use alloc::ffi::CString;
use lilium_sys::result::{Error, Result};
use lilium_sys::sys::handle::{self, HandlePtr};
use lilium_sys::sys::kstr::KStrCPtr;
use lilium_sys::sys::{fs as sys, io};
use rustix::fd::{BorrowedFd, IntoRawFd};
use rustix::fs::{Mode, OFlags};
use wl_impl::handle_base::{self, Handle};
use wl_impl::helpers::linux_error_to_lilium;
use wl_impl::{export_syscall, libc};

export_syscall! {
    unsafe extern fn OpenFile(ohdl: *mut HandlePtr<sys::FileHandle>, resolution_base: HandlePtr<sys::FileHandle>, path: KStrCPtr, opts: *const sys::FileOpenOptions) -> Result<()> {
        let dirfd = if resolution_base == HandlePtr::null() {
            rustix::fs::CWD
        } else {
            let res_base = unsafe { Handle::try_deref(resolution_base.cast())? };
            res_base.check_type(handle::HANDLE_SUBTYPE_IO_FILE as usize, 0)?;
            res_base.borrow_fd().ok_or(Error::UnsupportedOperation)?
        };

        let opts = unsafe{&*opts};

        let mut oflags = OFlags::CLOEXEC;

        if opts.op_mode == sys::OP_NO_ACCESS || opts.op_mode == sys::OP_ACL_ACCESS {
            oflags |= OFlags::PATH;
        }
        else if (opts.access_mode & (sys::ACCESS_READ | sys::ACCESS_WRITE)) ==  (sys::ACCESS_READ | sys::ACCESS_WRITE){
            oflags |= OFlags::RDWR
        } else if (opts.access_mode & sys::ACCESS_READ) == 0 {
            oflags |= OFlags::RDONLY
        } else if (opts.access_mode & sys::ACCESS_WRITE) == 0 {
            oflags |= OFlags::WRONLY
        }

        if ( opts.access_mode & sys::ACCESS_CREATE) == 0 {
            oflags |= OFlags::CREATE
        }

        if (opts.access_mode & sys::ACCESS_CREATE_EXCLUSIVE) == 0 {
            oflags |= OFlags::EXCL;
        }

        if (opts.access_mode & sys::ACCESS_LINK_STREAM_DIRECT) == 0 {
            oflags |= OFlags::NOFOLLOW
        }

        if (opts.access_mode & sys::ACCESS_TRUNCATE) == 0 {
            oflags |= OFlags::TRUNC
        }

        if (opts.access_mode & sys::ACCESS_START_END) == 0 {
            oflags |= OFlags::APPEND
        }

        if opts.op_mode == sys::OP_DIRECTORY_ACCESS {
            oflags |= OFlags::DIRECTORY;
        }

        let path = CString::new(unsafe { path.as_str() })
            .or_else(|_| Error::from_code(-0x801).map(|_| unreachable!()))?;

        let mode = Mode::from_raw_mode(0o666);

        let fd = rustix::fs::openat(dirfd, &*path, oflags, mode)
            .map_err(|v| linux_error_to_lilium(unsafe { linux_errno::Error::new_unchecked(v.raw_os_error() as u16)}))?;

        let fd = fd.into_raw_fd() as i64;


        let hdl = Handle {
            ty: handle::HANDLE_SUBTYPE_IO_FILE as usize,
            blob1: core::ptr::null_mut(),
            blob2: core::ptr::null_mut(),
            fd
        };

        let ptr = handle_base::insert_handle(hdl)?;

        unsafe { ohdl.write(ptr.cast()); }

        Ok(())
    }
}
