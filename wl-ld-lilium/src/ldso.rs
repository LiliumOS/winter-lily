use core::ffi::{CStr, c_void};

use alloc::vec::Vec;
use ld_so_impl::arch::crash_unrecoverably;
use ld_so_impl::elf::consts::{ELFCLASS64, ELFMAG, ElfIdent};
use ld_so_impl::elf::{EM_HOST, ElfClass, ElfHeader, ElfHost};
use ld_so_impl::helpers::{cstr_from_ptr, strlen_impl};
use ld_so_impl::hidden_syms;
use ld_so_impl::loader::{Error, LoaderImpl};
use ld_so_impl::resolver::DynEntry;
use linux_errno::{EACCES, ENOENT};
use linux_raw_sys::general::AT_FDCWD;
use linux_syscall::{Result as _, SYS_close, SYS_open, SYS_write, syscall};

use bytemuck::Zeroable;
use wl_interface_map::wl_init_subsystem_name;

use crate::entry::RESOLVER;
use crate::env::{self, get_env};

use crate::helpers::{
    FusedUnsafeCell, MmapAllocator, OnceLock, SyncPointer, copy_to_slice_head, debug, has_prefix,
    safe_zeroed,
};

use crate::io::BufFdReader;

pub static __MMAP_ADDR: FusedUnsafeCell<SyncPointer<*mut c_void>> =
    FusedUnsafeCell::new(SyncPointer::null_mut());

pub static __LDSO_HOST_SEARCH_LIST: OnceLock<&str> = OnceLock::new();
pub static __LDSO_LILIUM_SEARCH_LIST: OnceLock<&str> = OnceLock::new();

use crate::helpers::{expand_glob, open_sysroot_rdonly};

fn read_config_file(fd: i32, buf: &mut Vec<u8, MmapAllocator>) -> crate::io::Result<()> {
    let mut v = safe_zeroed::<[u8; 256]>();
    let mut file = BufFdReader::new(fd);

    loop {
        let str = match file.read_line_static(&mut v)? {
            Some(val) => val,
            None => break,
        };

        eprintln!("{str}");

        let st = SplitAscii::new(str, b'#').split_once().0.trim_ascii();

        if st.is_empty() {
            continue;
        }

        if let Some(path) = st.strip_prefix("include ") {
            match open_sysroot_rdonly(linux_raw_sys::general::AT_FDCWD, path) {
                Ok(fd) => read_config_file(fd, buf)?,
                Err(e) => {
                    if SplitAscii::new(path, b'*').find().is_some() {
                        expand_glob(path, |dirfd, path| {
                            let fd = open_sysroot_rdonly(dirfd, unsafe {
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
    res.check()
}

use crate::helpers::SplitAscii;
use crate::loader::LOADER;

#[inline(never)]
fn init_cache_slow(env_name: &str, config_path: &str) -> crate::io::Result<&'static str> {
    let mut buf = Vec::with_capacity_in(4096, MmapAllocator::new_with_hint(__MMAP_ADDR.0));

    for v in get_env(env_name)
        .iter()
        .flat_map(|v| SplitAscii::new(v, b':'))
    {
        let pos = buf.len();
        buf.resize(pos + v.len() + 1, 0x1E);
        copy_to_slice_head(&mut buf[pos..], v.as_bytes());
    }
    if let Ok(fd) = open_sysroot_rdonly(linux_raw_sys::general::AT_FDCWD, config_path) {
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

pub fn open_module(search: SearchType, name: &CStr) -> crate::io::Result<i32> {
    let mut buf = Vec::with_capacity_in(
        4096,
        MmapAllocator::new_with_hint(__MMAP_ADDR.0.wrapping_add(4096 * 8)),
    );
    unsafe {
        buf.set_len(4096);
    }

    let path = match search {
        SearchType::Host => __LDSO_HOST_SEARCH_LIST
            .get_or_try_init(|| {
                init_cache_slow(
                    "LD_LIBRARY_PATH_WL_HOST",
                    get_env("WL_NATIVE_LD_SO_CONF").unwrap_or("/etc/ld.so.conf"),
                )
            })
            .copied(),
        SearchType::Winter => __LDSO_LILIUM_SEARCH_LIST
            .get_or_try_init(|| {
                init_cache_slow(
                    "LD_LIBRARY_PATH_WL_LILIUM",
                    get_env("WL_SYSROOT_LD_SO_CONF").unwrap_or("/etc/ld-lilium.so.conf"),
                )
            })
            .copied(),
    };

    let path = path?;

    for p in SplitAscii::new(path, b'\x1E') {
        eprintln!("{}: Searching: {p}", unsafe {
            core::str::from_utf8_unchecked(name.to_bytes())
        });
        let vbuf = copy_to_slice_head(&mut buf, p.as_bytes());
        vbuf[0] = b'/';
        let vbuf = copy_to_slice_head(&mut vbuf[1..], name.to_bytes());
        if vbuf.is_empty() {
            panic!()
        }
        vbuf[0] = 0;

        let bname = unsafe { cstr_from_ptr(buf.as_ptr().cast()) };

        let fd = unsafe { syscall!(SYS_open, buf.as_ptr(), linux_raw_sys::general::O_RDONLY) };

        match fd.check() {
            Ok(()) => {
                let fd = fd.as_u64_unchecked() as i32;
                let mut header = bytemuck::zeroed::<ElfHeader<ElfHost>>();

                let mut rd = BufFdReader::new(fd);

                match rd.read_exact(bytemuck::bytes_of_mut(&mut header.e_ident)) {
                    Ok(()) => {}
                    Err(e) => continue,
                }

                eprintln!("{header:#x?}");

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

                rd.seek(crate::io::SeekFrom::Start(0))?;
                core::mem::forget(rd);

                debug(
                    "open_module(found)",
                    unsafe { cstr_from_ptr(buf.as_ptr().cast()) }.to_bytes(),
                );
                return Ok(fd);
            }
            Err(e) => match e {
                ENOENT | EACCES => continue,
                v => return Err(v),
            },
        }
    }

    Err(ENOENT)
}

pub type Result<T> = core::result::Result<T, ld_so_impl::loader::Error>;

pub fn load_subsystem(name: &'static str, winter_soname: &'static CStr) -> &'static DynEntry {
    let udata = core::ptr::without_provenance_mut(SearchType::Host as usize);
    let mut var_name = [0u8; 96];
    let next = copy_to_slice_head(&mut var_name, "WL_SUBSYS_".as_bytes());
    let len = 96 - copy_to_slice_head(next, name.as_bytes()).len();

    let env_name = unsafe { core::str::from_utf8_unchecked(&var_name[..len]) };

    let fhdl = if let Some(var) = env::get_env(env_name) {
        if var.contains('/') {
            let fd = open_sysroot_rdonly(AT_FDCWD, var)
                .unwrap_or_else(|_| RESOLVER.resolve_error(winter_soname, Error::ObjectNotFound));

            core::ptr::without_provenance_mut(fd as usize)
        } else {
            let override_soname = env::get_cenv(env_name).expect("Expected an env var");

            unsafe {
                LOADER.find(override_soname, udata).unwrap_or_else(|_| {
                    RESOLVER.resolve_error(winter_soname, Error::ObjectNotFound)
                })
            }
        }
    } else {
        let next = copy_to_slice_head(&mut var_name, "libwl-lilium-".as_bytes());
        let next = copy_to_slice_head(next, name.as_bytes());
        copy_to_slice_head(next, ".so".as_bytes());

        let soname = CStr::from_bytes_until_nul(&var_name).unwrap();

        unsafe {
            LOADER
                .find(soname, udata)
                .unwrap_or_else(|_| RESOLVER.resolve_error(winter_soname, Error::ObjectNotFound))
        }
    };

    unsafe { RESOLVER.load_from_handle(Some(winter_soname), udata, fhdl) }
}

pub fn load_and_init_subsystem(
    name: &'static str,
    winter_soname: &'static CStr,
) -> &'static DynEntry {
    let ent = load_subsystem(name, winter_soname);

    let init_subsystem = RESOLVER.find_sym_in(wl_init_subsystem_name!(C), ent);
    eprintln!("Found libusi-{name}.so:__init_subsystem {init_subsystem:p}");

    let init_subsystem: wl_interface_map::InitSubsystemTy =
        unsafe { core::mem::transmute(init_subsystem) };

    init_subsystem();

    ent
}
