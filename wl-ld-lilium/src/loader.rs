use core::{ffi::c_void, sync::atomic::AtomicPtr};

use ld_so_impl::{
    elf::{
        ElfOffset, ElfSize,
        consts::{self, PT_LOAD},
    },
    loader::{Error, LoaderImpl},
};
use linux_syscall::{Result as _, SYS_lseek, SYS_mmap, SYS_mprotect, SYS_read, syscall};

use crate::{
    helpers::debug,
    ldso::{self, SearchType},
};

pub struct FdLoader {
    pub native_base: AtomicPtr<c_void>,
    pub winter_base: AtomicPtr<c_void>,
}

impl LoaderImpl for FdLoader {
    unsafe fn find(
        &self,
        soname: &std::ffi::CStr,
        udata: *mut std::ffi::c_void,
    ) -> Result<*mut c_void, Error> {
        // Safety: `lookup_soname` is only ever called using a provided udata, which is `core::ptr::without_provenance(SearchType as usize)`
        let search: SearchType = unsafe { core::mem::transmute(udata) };

        let (search, nname) = if search == SearchType::Winter {
            match soname.to_bytes() {
                b"libusi-base.so" | b"libusi-thread.so" | b"libusi-io.so"
                | b"libusi-process.so" | b"libusi-debug.so" | b"libusi-kmgmt.so" => {
                    // default subsystem, preloaded before any lilium code is loaded
                    return Err(Error::AssumePresent);
                }
                b"libusi-rtld.so" | b"libusi-unwind.so" | b"libusi-vti.so" | b"libc.so"
                | b"libdl.so" => {
                    let redirect_name = match soname.to_bytes() {
                        b"libusi-rtld.so" => c"libwl-usi-rtld.so",
                        b"libusi-unwind.so" => c"libwl-usi-unwind.so",
                        b"libusi-vti.so" => c"libwl-usi-vti.so",
                        b"libc.so" => c"libwl-usi-posix.so",
                        b"libdl.so" => c"libwl-usi-dl.so",
                        _ => todo!(),
                    };

                    (SearchType::Host, redirect_name)
                }
                _ => (search, soname),
            }
        } else {
            (search, soname)
        };
        ldso::open_module(search, nname)
            .map_err(|_| Error::Fatal)
            .map(|v| core::ptr::without_provenance_mut(v as usize))
    }

    unsafe fn map_phdrs(
        &self,
        phdr: &[ld_so_impl::elf::ElfPhdr],
        map_desc: *mut c_void,
        base_addr: *mut std::ffi::c_void,
    ) -> Result<*mut std::ffi::c_void, Error> {
        debug("map_phdrs", b"entry");
        for phdr in phdr {
            if phdr.p_type != PT_LOAD {
                continue;
            }

            let paddr = base_addr.wrapping_offset(phdr.p_paddr as isize);

            let off = phdr.p_offset;
            let file_len = phdr.p_filesz as usize;

            let ptr = unsafe { core::slice::from_raw_parts_mut(paddr.cast(), file_len) };

            self.read_offset(off, map_desc, ptr)?;
        }

        let mut last_pg_addr = base_addr;
        let mut last_perms = 0;

        for phdr in phdr {
            let mut addr = base_addr.wrapping_offset(phdr.p_paddr as isize);
            let mut len = phdr.p_memsz as usize;

            let mut perms = libc::PROT_READ;

            if (phdr.p_flags & consts::PF_W) != 0 {
                perms |= libc::PROT_WRITE;
            } else if (phdr.p_flags & consts::PF_X) != 0 {
                perms |= libc::PROT_EXEC;
            }

            if addr.map_addr(|v| v & !4095) <= last_pg_addr {
                let res = unsafe { syscall!(SYS_mprotect, last_pg_addr, 4096, perms | last_perms) };
                res.check().map_err(|_| Error::Fatal)?;
                len = len.saturating_sub(4096);
                addr = addr.map_addr(|v| (v + 4095) & !4095);
            }

            if len > 0 {
                let res = unsafe { syscall!(SYS_mprotect, addr, len, perms) };
                res.check().map_err(|_| Error::Fatal)?;

                let end = addr.wrapping_add(len);
                last_pg_addr = end.map_addr(|v| (v - 1) & !4095);
                last_perms = perms;
            }
        }

        Ok(base_addr)
    }

    fn read_offset(
        &self,
        off: ld_so_impl::elf::ElfOffset,
        map_desc: *mut c_void,
        mut sl: &mut [u8],
    ) -> Result<(), Error> {
        let fd = map_desc.addr() as i32;
        let res = unsafe { syscall!(SYS_lseek, fd, off as libc::off_t, libc::SEEK_SET) };
        res.check().map_err(|_| Error::Fatal)?;

        while !sl.is_empty() {
            let res = unsafe { syscall!(SYS_read, fd, sl.as_mut_ptr(), sl.len()) };

            match res.check().map_err(|e| e.get() as i32) {
                Ok(()) => {
                    sl = &mut sl[..res.as_usize_unchecked()];
                }
                Err(libc::EINTR) => continue,
                Err(_) => return Err(Error::Fatal),
            }
        }
        Ok(())
    }

    unsafe fn alloc_base_addr(
        &self,
        udata: *mut c_void,
        max_pma: ld_so_impl::elf::ElfAddr,
    ) -> Result<*mut c_void, Error> {
        // Safety: `lookup_soname` is only ever called using a provided udata, which is `core::ptr::without_provenance(SearchType as usize)`
        let search: SearchType = unsafe { core::mem::transmute(udata) };
        let base = match search {
            SearchType::Host => &self.native_base,
            SearchType::Winter => &self.winter_base,
        };

        let addr = base.fetch_byte_add(
            (max_pma as usize + 4095) & !4095,
            std::sync::atomic::Ordering::Relaxed,
        );
        // Eventually I'll do better loading opts.
        let res = unsafe {
            syscall!(
                SYS_mmap,
                addr,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED,
                -1i32,
                0
            )
        };

        res.check().map_err(|_| Error::Fatal)?;

        Ok(unsafe { core::ptr::with_exposed_provenance_mut(res.as_usize_unchecked()) })
    }
}

pub static LOADER: FdLoader = FdLoader {
    native_base: AtomicPtr::new(core::ptr::null_mut()),
    winter_base: AtomicPtr::new(core::ptr::null_mut()),
};
