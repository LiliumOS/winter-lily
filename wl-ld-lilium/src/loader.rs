use core::{
    ffi::c_void,
    fmt::Write,
    sync::atomic::{AtomicPtr, AtomicUsize},
};

use ld_so_impl::{
    elf::{
        ElfOffset, ElfSize,
        consts::{self, PT_LOAD},
    },
    loader::{Error, LoaderImpl},
};
use linux_errno::EINTR;
use linux_raw_sys::general::{__kernel_off_t, ARCH_SET_FS, PROT_READ, PROT_WRITE};
use linux_syscall::{
    Result as _, SYS_arch_prctl, SYS_lseek, SYS_mmap, SYS_mprotect, SYS_read, syscall,
};

use crate::{
    entry::TLS_BLOCK_SIZE,
    helpers::debug,
    io::STDERR,
    is_x86_feature_detected,
    ldso::{self, SearchType},
};

pub struct FdLoader {
    pub native_base: AtomicPtr<c_void>,
    pub winter_base: AtomicPtr<c_void>,
    pub tls_off: AtomicUsize,
}

impl LoaderImpl for FdLoader {
    unsafe fn find(
        &self,
        soname: &core::ffi::CStr,
        udata: *mut core::ffi::c_void,
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
            .map_err(|_| Error::ObjectNotFound)
            .map(|v| core::ptr::without_provenance_mut(v as usize))
    }

    unsafe fn map_phdrs(
        &self,
        phdr: &[ld_so_impl::elf::ElfPhdr],
        map_desc: *mut c_void,
        base_addr: *mut core::ffi::c_void,
    ) -> Result<*mut core::ffi::c_void, Error> {
        debug("map_phdrs", b"entry");
        let mut last_addr: *mut c_void = core::ptr::without_provenance_mut(0);
        let mut last_perms = 0;

        for phdr in phdr {
            let mut addr = base_addr.wrapping_offset(phdr.p_paddr as isize);
            let mut len = phdr.p_memsz as usize;
            let mut file_len = phdr.p_filesz as usize;
            let mut offset = phdr.p_offset as usize;

            let mut perms = linux_raw_sys::general::PROT_READ;

            if (phdr.p_flags & consts::PF_W) != 0 {
                perms |= linux_raw_sys::general::PROT_WRITE;
            } else if (phdr.p_flags & consts::PF_X) != 0 {
                perms |= linux_raw_sys::general::PROT_EXEC;
            }

            let last_pg_addr = last_addr.map_addr(|v| v & !4095);

            if addr.map_addr(|v| v & !4095) <= last_pg_addr {
                let res = unsafe {
                    syscall!(
                        SYS_mprotect,
                        last_pg_addr,
                        4096,
                        linux_raw_sys::general::PROT_READ | linux_raw_sys::general::PROT_WRITE
                    )
                };
                res.check().map_err(|_| Error::LoadError)?;
                let size = addr.align_offset(4096).min(file_len);
                let ptr = unsafe { core::slice::from_raw_parts_mut(addr.cast::<u8>(), size) };
                self.read_offset(phdr.p_offset, map_desc, ptr)?;

                let res = unsafe { syscall!(SYS_mprotect, last_pg_addr, 4096, perms | last_perms) };
                res.check().map_err(|_| Error::LoadError)?;
                len = len.saturating_sub(4096);
                file_len = file_len.saturating_sub(4096);
                addr = addr.map_addr(|v| (v + 4095) & !4095);
                offset = (offset + 4095) & !4095;
            }

            if file_len > 0 {
                if addr.addr() & 4095 != (phdr.p_offset as usize) & 4095 {
                    todo!()
                }

                let f_offset = offset & !4095;

                let extra_len = offset - f_offset;
                eprintln!(
                    "mmap({addr:p} (adjusted {:p}), {file_len:#018x} (adjusted: {:#018x}), {perms:03b}, MAP_PRIVATE | MAP_FIXED, {}, {:#018x} (adjusted: {:#018x})",
                    addr.map_addr(|v| v & !4095),
                    extra_len + file_len,
                    map_desc.addr() as i32,
                    phdr.p_offset,
                    phdr.p_offset & !4095,
                );
                let res = unsafe {
                    syscall!(
                        SYS_mmap,
                        addr.map_addr(|v| v & !4095),
                        extra_len + file_len,
                        PROT_READ | PROT_WRITE,
                        linux_raw_sys::general::MAP_PRIVATE | linux_raw_sys::general::MAP_FIXED,
                        map_desc.addr() as i32,
                        phdr.p_offset & !4095
                    )
                };
                res.check().map_err(|_| Error::LoadError)?;

                let file_start = addr.map_addr(|v| v & !4095);

                unsafe {
                    core::ptr::write_bytes(file_start, 0, addr.offset_from_unsigned(file_start));
                }

                let file_end = addr.wrapping_add(file_len);

                let end = addr.wrapping_add(len);

                unsafe {
                    core::ptr::write_bytes(end, 0, end.offset_from_unsigned(file_end));
                }

                let res = unsafe {
                    syscall!(
                        SYS_mprotect,
                        addr.map_addr(|v| v & !4095),
                        extra_len + len,
                        perms
                    )
                };
                res.check().map_err(|_| Error::LoadError)?;

                last_addr = end.wrapping_sub(1);
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
        let res = unsafe {
            syscall!(
                SYS_lseek,
                fd,
                off as __kernel_off_t,
                linux_raw_sys::general::SEEK_SET
            )
        };
        res.check().map_err(|_| Error::ReadError)?;

        while !sl.is_empty() {
            let res = unsafe { syscall!(SYS_read, fd, sl.as_mut_ptr(), sl.len()) };

            match res.check() {
                Ok(()) => {
                    let num = res.as_usize_unchecked();
                    if num == 0 {
                        return Err(Error::ReadError);
                    }
                    sl = &mut sl[num..];
                }
                // Err(EINTR) => continue,
                Err(_) => return Err(Error::ReadError),
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

        let length = (max_pma as usize + 4095) & !4095;

        let addr = base.fetch_byte_add(length, core::sync::atomic::Ordering::Relaxed);
        eprintln!("Load adder {addr:p}");
        // Eventually I'll do better loading opts.
        let res = unsafe {
            syscall!(
                SYS_mmap,
                addr,
                length,
                linux_raw_sys::general::PROT_READ | linux_raw_sys::general::PROT_WRITE,
                linux_raw_sys::general::MAP_PRIVATE
                    | linux_raw_sys::general::MAP_ANONYMOUS
                    | linux_raw_sys::general::MAP_FIXED_NOREPLACE,
                -1i32,
                0
            )
        };

        res.check().map_err(|_| Error::AllocError)?;

        Ok(unsafe { core::ptr::with_exposed_provenance_mut(res.as_usize_unchecked()) })
    }

    fn write_str(&self, st: &str) -> core::fmt::Result {
        { STDERR }.write_str(st)
    }

    fn alloc_tls(&self, tls_size: usize) -> Result<usize, Error> {
        let val = self
            .tls_off
            .fetch_add(tls_size, core::sync::atomic::Ordering::Relaxed);

        if (val + tls_size) > (TLS_BLOCK_SIZE >> 1) {
            return Err(Error::AllocError);
        }

        let pg = get_tp().map_addr(|a| a + (val & !4095));

        let res = unsafe { syscall!(SYS_mprotect, pg, tls_size, PROT_READ | PROT_WRITE) };

        res.check().map_err(|_| Error::AllocError)?;
        Ok(val)
    }

    fn tls_direct_offset(&self, module: usize) -> Result<usize, Error> {
        Ok(module) // every module is valid for offset
    }

    fn load_tls(
        &self,
        tls_module: usize,
        desc: *mut c_void,
        off: ElfOffset,
        sz: ElfSize,
    ) -> Result<(), Error> {
        // TODO: Load Master Copy and mark as dirty for other threads
        let tp = get_tp();

        let module = tp.wrapping_add(tls_module);

        let sl = unsafe { core::slice::from_raw_parts_mut(module.cast::<u8>(), sz as usize) };

        self.read_offset(off, desc, sl)
    }
}

#[repr(C)]
pub struct Tcb {
    tls_base: *mut c_void,
}

pub static LOADER: FdLoader = FdLoader {
    native_base: AtomicPtr::new(core::ptr::null_mut()),
    winter_base: AtomicPtr::new(core::ptr::null_mut()),
    tls_off: AtomicUsize::new(core::mem::size_of::<Tcb>()),
};

pub fn set_tp(ptr: *mut c_void) {
    unsafe {
        ptr.cast::<Tcb>().write(Tcb { tls_base: ptr });
    }
    cfg_match::cfg_match! {
        target_arch = "x86_64" => if is_x86_feature_detected!("fsgsbase"){
            unsafe { core::arch::asm!("wrfsbase {ptr}", ptr = in(reg) ptr, options(preserves_flags, nostack))}
        } else {
            unsafe { let _ = syscall!(SYS_arch_prctl, ARCH_SET_FS, ptr.expose_provenance());}
        },

    }
}

pub fn get_tp() -> *mut c_void {
    let val: *mut c_void;
    cfg_match::cfg_match! {
        target_arch = "x86_64" =>
            unsafe { core::arch::asm!("mov {val}, fs:[{val}]", val = inout(reg) 0usize => val, options(readonly, pure, preserves_flags, nostack))}
        ,
    }
    val
}
