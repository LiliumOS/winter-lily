use std::ffi::{CStr, c_void};
use std::io::{Read, Seek};

use ld_so_impl::arch::crash_unrecoverably;
use ld_so_impl::elf::consts::{ELFCLASS64, ELFMAG, ElfIdent};
use ld_so_impl::elf::{EM_HOST, ElfClass, ElfHeader, ElfHost};
use ld_so_impl::helpers::{cstr_from_ptr, strlen_impl};
use ld_so_impl::hidden_syms;
use linux_syscall::{Result as _, SYS_close, SYS_open, SYS_write, syscall};

use bytemuck::Zeroable;

use crate::env::get_env;

use crate::helpers::{
    FusedUnsafeCell, MmapAllocator, OnceLock, SyncPointer, copy_to_slice_head, debug, has_prefix,
    safe_zeroed,
};

use crate::io::{BufFdReader, linux_err_into_io_err};

pub static __MMAP_ADDR: FusedUnsafeCell<SyncPointer<*mut c_void>> =
    FusedUnsafeCell::new(SyncPointer::null_mut());

pub static __LDSO_HOST_SEARCH_LIST: OnceLock<&str> = OnceLock::new();
pub static __LDSO_LILIUM_SEARCH_LIST: OnceLock<&str> = OnceLock::new();

use crate::helpers::{expand_glob, open_rdonly};

fn read_config_file(fd: i32, buf: &mut Vec<u8, MmapAllocator>) -> std::io::Result<()> {
    let mut v = safe_zeroed::<[u8; 256]>();
    let mut file = BufFdReader::new(fd);

    loop {
        let str = match file.read_line_static(&mut v)? {
            Some(val) => val,
            None => break,
        };

        let st = SplitAscii::new(str, b'#').split_once().0.trim_ascii();

        if st.is_empty() {
            continue;
        }

        if let Some(path) = st.strip_prefix("include ") {
            match open_rdonly(libc::AT_FDCWD, path) {
                Ok(fd) => read_config_file(fd, buf)?,
                Err(e) => {
                    if SplitAscii::new(path, b'*').find().is_some() {
                        expand_glob(path, |dirfd, path| {
                            let fd = open_rdonly(dirfd, unsafe {
                                core::str::from_utf8_unchecked(path.to_bytes())
                            })?;

                            read_config_file(fd, buf)
                        })?;
                    } else {
                        return Err(e);
                    }
                }
            }
        } else {
            let pos = buf.len();
            buf.resize(pos + st.len() + 1, 0x1E);
            copy_to_slice_head(&mut buf[pos..], st.as_bytes());
        }
    }

    let res = unsafe { syscall!(SYS_close, fd) };
    res.check().map_err(linux_err_into_io_err)
}

use crate::helpers::SplitAscii;

#[inline(never)]
fn init_cache_slow(env_name: &str, config_path: &str) -> std::io::Result<&'static str> {
    let mut buf = Vec::with_capacity_in(4096, MmapAllocator::new_with_hint(__MMAP_ADDR.0));

    for v in get_env(env_name)
        .iter()
        .flat_map(|v| SplitAscii::new(v, b':'))
    {
        let pos = buf.len();
        buf.resize(pos + v.len() + 1, 0x1E);
        copy_to_slice_head(&mut buf[pos..], v.as_bytes());
    }
    if let Ok(fd) = open_rdonly(libc::AT_FDCWD, config_path) {
        read_config_file(fd, &mut buf)?;
    }

    Ok(unsafe { core::str::from_utf8_unchecked(buf.leak()) })
}

hidden_syms!(init_cache_slow);

#[repr(usize)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum SearchType {
    Host,
    Winter = 1,
}

pub fn open_module(search: SearchType, name: &CStr) -> std::io::Result<i32> {
    let mut buf = Vec::with_capacity_in(
        4096,
        MmapAllocator::new_with_hint(__MMAP_ADDR.0.wrapping_add(4096 * 8)),
    );
    unsafe {
        buf.set_len(4096);
    }

    let path = match search {
        SearchType::Host => __LDSO_HOST_SEARCH_LIST
            .get_or_try_init(|| init_cache_slow("LD_LIBRARY_PATH_WL_HOST", "/etc/ld.so.conf"))
            .copied(),
        SearchType::Winter => __LDSO_LILIUM_SEARCH_LIST
            .get_or_try_init(|| {
                init_cache_slow(
                    "LD_LIBRARY_PATH_WL_LILIUM",
                    get_env("WL_SYSROOT_LD_SO_CONF").unwrap_or("ld-lilium.so.conf"),
                )
            })
            .copied(),
    };

    let path = path?;

    for p in SplitAscii::new(path, b'\x1E') {
        let vbuf = copy_to_slice_head(&mut buf, p.as_bytes());
        vbuf[0] = b'/';

        if copy_to_slice_head(&mut vbuf[1..], name.to_bytes()).is_empty() {
            panic!()
        }

        let bname = unsafe { cstr_from_ptr(buf.as_ptr().cast()) };
        debug("open_module:search", bname.to_bytes());

        let fd = unsafe { syscall!(SYS_open, buf.as_ptr(), libc::O_RDONLY) };

        match fd.check() {
            Ok(()) => {
                let fd = fd.as_u64_unchecked() as i32;
                let mut header = bytemuck::zeroed::<ElfHeader<ElfHost>>();

                let mut rd = BufFdReader::new(fd);

                match rd.read_exact(bytemuck::bytes_of_mut(&mut header.e_ident)) {
                    Ok(()) => {}
                    Err(e) => continue,
                }

                if header.e_ident.ei_class != ElfHost::EI_CLASS {
                    continue;
                }
                match rd.read_exact(&mut bytemuck::bytes_of_mut(&mut header)[16..]) {
                    Ok(()) => {}
                    Err(e) => continue,
                }

                if header.e_machine != EM_HOST {
                    continue;
                }

                rd.seek(std::io::SeekFrom::Start(0))?;
                core::mem::forget(rd);

                crate::entry::x86_64::RESOLVER
                    .resolve_error(unsafe { cstr_from_ptr(buf.as_ptr().cast()) }); // Debug print this
                return Ok(fd);
            }
            Err(e) => match e.get() as i32 {
                libc::ENOENT | libc::EACCES => continue,
                v => return Err(std::io::Error::from_raw_os_error(v)),
            },
        }
    }

    Err(std::io::Error::from_raw_os_error(libc::ENOENT))
}
