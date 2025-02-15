use std::ffi::{CStr, c_void};

use ld_so_impl::arch::crash_unrecoverably;
use linux_syscall::{Result as _, SYS_close, SYS_mmap, SYS_open, Syscall, syscall};

#[cfg(target_arch = "x86_64")]
pub use linux_errno::Error as SysError;

use crate::env::{self, get_env};

use crate::helpers::{FusedUnsafeCell, MmapAllocator, OnceLock, SyncPointer, copy_to_slice_head};

use crate::io::{BufFdReader, linux_err_into_io_err};

pub static __MMAP_ADDR: FusedUnsafeCell<SyncPointer<*mut c_void>> =
    FusedUnsafeCell::new(SyncPointer::null_mut());

static __LDSO_HOST_SEARCH_LIST: OnceLock<&str> = OnceLock::new();
static __LDSO_LILIUM_SEARCH_LIST: OnceLock<&str> = OnceLock::new();

fn open_rdonly(st: &str) -> std::io::Result<i32> {
    let mut path = [0u8; 256];
    copy_to_slice_head(&mut path, st.as_bytes())[0] = 0;
    let fd = unsafe { syscall!(SYS_open, path.as_ptr(), libc::O_RDONLY) };
    fd.check().map_err(linux_err_into_io_err)?;

    Ok(fd.as_u64_unchecked() as i32)
}

fn read_config_file(fd: i32, buf: &mut Vec<u8, MmapAllocator>) -> std::io::Result<()> {
    let mut v = [0u8; 256];
    let mut file = BufFdReader::new(fd);

    loop {
        let str = match file.read_line_static(&mut v)? {
            Some(val) => val.unwrap_or_else(|_| crash_unrecoverably()),
            None => break,
        };

        let st = str
            .split_once("#")
            .map_or(str as &str, |(l, _)| l)
            .trim_ascii();

        if st.is_empty() {
            continue;
        }

        if let Some(path) = st.strip_prefix("include ") {
            let fd = open_rdonly(path)?;

            read_config_file(fd, buf)?;
        } else {
            let pos = buf.len();
            buf.resize(pos + st.len() + 1, 0x1E);
            copy_to_slice_head(&mut buf[pos..], st.as_bytes());
        }
    }

    let res = unsafe { syscall!(SYS_close, fd) };
    res.check().map_err(linux_err_into_io_err)
}

fn init_cache_slow(env_name: &str, config_path: &str) -> std::io::Result<&'static str> {
    let mut buf = Vec::with_capacity_in(4096, MmapAllocator::new_with_hint(__MMAP_ADDR.0));

    for v in get_env(env_name).iter().flat_map(|v| v.split(':')) {
        let pos = buf.len();
        buf.resize(pos + v.len() + 1, 0x1E);
        copy_to_slice_head(&mut buf[pos..], v.as_bytes());
    }
    if let Ok(fd) = open_rdonly(config_path) {
        read_config_file(fd, &mut buf)?;
    }

    Ok(unsafe { core::str::from_utf8_unchecked(buf.leak()) })
}
