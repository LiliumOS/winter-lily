use core::{
    ffi::c_void,
    fmt::Write,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use ld_so_impl::{
    arch::crash_unrecoverably,
    elf::{
        ElfOffset, ElfSize,
        consts::{self, PT_LOAD},
    },
    loader::{Error, LoaderImpl},
};
use linux_errno::EINTR;
use linux_raw_sys::general::{__kernel_off_t, ARCH_SET_FS, PROT_READ, PROT_WRITE};
use linux_syscall::{
    Result as _, SYS_arch_prctl, SYS_close, SYS_lseek, SYS_mmap, SYS_mprotect, SYS_read, syscall,
};
use wl_helpers::sync::RwLock;

use crate::{
    entry::TLS_BLOCK_SIZE,
    helpers::{FusedUnsafeCell, SyncPointer, is_x86_feature_detected},
    io::STDERR,
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
                let adjusted_len = extra_len + file_len;
                let res = unsafe {
                    syscall!(
                        SYS_mmap,
                        addr.map_addr(|v| v & !4095),
                        adjusted_len,
                        PROT_READ | PROT_WRITE,
                        linux_raw_sys::general::MAP_PRIVATE | linux_raw_sys::general::MAP_FIXED,
                        map_desc.addr() as i32,
                        f_offset,
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
                    core::ptr::write_bytes(file_end, 0, end.offset_from_unsigned(file_end));
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

        Ok(core::ptr::with_exposed_provenance_mut(
            res.as_usize_unchecked(),
        ))
    }

    fn write_str(&self, st: &str) -> core::fmt::Result {
        { STDERR }.write_str(st)
    }

    unsafe fn close_hdl(&self, hdl: *mut c_void) {
        let _ = unsafe { syscall!(SYS_close, hdl.addr() as i32) };
    }

    fn alloc_tls(&self, tls_size: usize, tls_align: usize, exec_tls: bool) -> Result<isize, Error> {
        let val = if !exec_tls {
            self.tls_off
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                    let next = ((v + (tls_align - 1)) & !(tls_align - 1)) + tls_size;
                    if next > (TLS_BLOCK_SIZE >> 1) {
                        return None;
                    } else {
                        return Some(next);
                    }
                })
                .map_err(|_| Error::AllocError)? as isize
        } else {
            (-(tls_size as isize)) & !(tls_align as isize - 1)
        };

        let aligned_val = (val + (tls_align as isize - 1)) & !(tls_align as isize - 1);

        let pg = TLS_MC.0.map_addr(|a| a.wrapping_add_signed(val & !4095));
        let map_len = tls_size as isize + (aligned_val - (val & !4095));

        let res = unsafe { syscall!(SYS_mprotect, pg, map_len, PROT_READ | PROT_WRITE) };
        res.check().map_err(|_| Error::AllocError)?;
        if !exec_tls {
            unsafe {
                (*get_master_tcb()).load_size += tls_size.wrapping_add_signed(aligned_val - val);
                (*get_master_tcb()).version += 1;
            }
            Ok(aligned_val)
        } else {
            unsafe {
                (*get_master_tcb()).dyn_size = (-aligned_val) as usize;
                (*get_master_tcb()).version += 1;
            }
            Ok(aligned_val)
        }
    }

    fn tls_direct_offset(&self, module: isize) -> Result<isize, Error> {
        Ok(module) // every module is valid for offset
    }

    unsafe fn load_tls(
        &self,
        tls_module: isize,
        laddr: *mut c_void,
        sz: ElfSize,
    ) -> Result<(), Error> {
        let tp = TLS_MC.0;

        let module = tp.wrapping_offset(tls_module);

        unsafe {
            core::ptr::copy_nonoverlapping(laddr.cast(), module, sz as usize);
        }
        Ok(())
    }
}

pub static TLS_MC: FusedUnsafeCell<SyncPointer<*mut c_void>> =
    FusedUnsafeCell::new(SyncPointer::null_mut());

pub fn get_master_tcb() -> *mut Tcb {
    TLS_MC.0.cast()
}

#[repr(C, align(32))]
#[derive(Debug)]
pub struct Tcb {
    tls_base: *mut c_void,
    pub load_size: usize,
    pub dyn_size: usize,
    pub version: usize,
}

pub static LOADER: FdLoader = FdLoader {
    native_base: AtomicPtr::new(core::ptr::null_mut()),
    winter_base: AtomicPtr::new(core::ptr::null_mut()),
    tls_off: AtomicUsize::new(core::mem::size_of::<Tcb>()),
};

pub static LOAD_LOCK: RwLock<()> = RwLock::new(());

/// # Safety
/// May be called at most once and before any threads are spawned
pub unsafe fn setup_tls_mc(tls_mc: *mut c_void) {
    unsafe {
        TLS_MC.as_ptr().write(SyncPointer(tls_mc));
    }
    unsafe {
        get_master_tcb().write(Tcb {
            tls_base: tls_mc,
            load_size: core::mem::size_of::<Tcb>(),
            dyn_size: 0,
            version: 0,
        })
    }
}

pub fn update_tls() {
    let _guard = LOAD_LOCK.read();

    let tp = get_tp();
    let tlsmc = TLS_MC.0;
    let tcb = unsafe { &mut *(tp.cast::<Tcb>()) };
    let mtcb = unsafe { &*tlsmc.cast::<Tcb>() };
    if tcb.load_size < mtcb.load_size {
        let len = mtcb.load_size - tcb.load_size;
        let pg = tp.map_addr(|a| a + (tcb.load_size & !4095));
        let map_len = mtcb.load_size - (tcb.load_size & !4095);
        let res = unsafe { syscall!(SYS_mprotect, pg, map_len, PROT_READ | PROT_WRITE) };

        if let Err(e) = res.check() {
            eprintln!("TCB Update (stls) failed: {e:?}");
            crash_unrecoverably()
        }

        unsafe {
            core::ptr::copy_nonoverlapping(tlsmc.add(tcb.load_size), tp.add(tcb.load_size), len);
        }
        tcb.load_size = mtcb.load_size;
    }

    if tcb.dyn_size < mtcb.dyn_size {
        let len = mtcb.dyn_size - tcb.dyn_size;
        let pg = tp.map_addr(|a| a - ((mtcb.dyn_size + 4095) & !4095));
        let map_len = ((mtcb.dyn_size + 4095) & !4095) - tcb.dyn_size;
        let res = unsafe { syscall!(SYS_mprotect, pg, map_len, PROT_READ | PROT_WRITE) };

        if let Err(e) = res.check() {
            eprintln!("TCB Update (dtls) failed: {e:?}");
            crash_unrecoverably()
        }

        unsafe {
            core::ptr::copy_nonoverlapping(tlsmc.sub(mtcb.dyn_size), tp.sub(mtcb.dyn_size), len);
        }
        tcb.dyn_size = mtcb.dyn_size;
    }

    tcb.version = mtcb.version;
}

pub fn set_tp(ptr: *mut c_void) {
    unsafe {
        ptr.cast::<Tcb>().write(Tcb {
            tls_base: ptr,
            load_size: core::mem::size_of::<Tcb>(),
            dyn_size: 0,
            version: 0,
        });
    }
    cfg_match::cfg_match! {
        target_arch = "x86_64" => if is_x86_feature_detected!("fsgsbase"){
            unsafe { core::arch::asm!("wrfsbase {ptr}", ptr = in(reg) ptr, options(preserves_flags, nostack))}
        } else {
            unsafe { let _ = syscall!(SYS_arch_prctl, ARCH_SET_FS, ptr.expose_provenance());}
        },
    }

    update_tls()
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
