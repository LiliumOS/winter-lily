use core::ffi::c_void;

use lilium_sys::{
    result::{Error, Result},
    sys::kstr::KCSlice,
    sys::process as sys,
};
use wl_impl::{
    export_syscall,
    helpers::{linux_error_to_lilium, read_checked, write_checked},
    libc::{self, mmap, mprotect, mremap, munmap},
};

export_syscall! {
    unsafe extern fn CreateMapping(base_addr: *mut *mut c_void, page_count: isize, map_attrs: u32, map_kind: u32, _map_ext: *const KCSlice<sys::MapExtendedAttr>) -> Result<()> {
        let hint_addr = unsafe { base_addr.read() };

        let mut linux_prot = 0;
        let mut linux_flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS;
        if (map_attrs & !(sys::MAP_ATTR_READ | sys::MAP_ATTR_WRITE | sys::MAP_ATTR_EXEC | sys::MAP_ATTR_THREAD_PRIVATE | sys::MAP_ATTR_PROC_PRIVATE | sys::MAP_ATTR_RESERVE)) != 0 ||
            (map_kind != sys::MAP_KIND_NORMAL && map_kind != sys::MAP_KIND_RESIDENT && map_kind != sys::MAP_KIND_SECURE && map_kind != sys::MAP_KIND_ENCRYPTED) {
            return Err(Error::InvalidOperation)
        }

        if (map_attrs & (sys::MAP_ATTR_WRITE | sys::MAP_ATTR_EXEC)) == (sys::MAP_ATTR_WRITE | sys::MAP_ATTR_EXEC) {
            // TODO: Check for MapExtendedAttrAllowWritableText
            return Err(Error::InvalidOperation)
        }

        if (map_attrs & sys::MAP_ATTR_READ) !=0 {
            linux_prot |= libc::PROT_READ;
        }

        if (map_attrs & sys::MAP_ATTR_WRITE) !=0  {
            linux_prot |= libc::PROT_WRITE;
        }

        if (map_attrs & sys::MAP_ATTR_EXEC) != 0 {
            linux_prot |= libc::PROT_EXEC
        }

        if (map_attrs & sys::MAP_ATTR_PROC_PRIVATE) != 0 {
            linux_flags |= libc::MAP_PRIVATE;
        }

        if (map_attrs & sys::MAP_ATTR_RESERVE) != 0 {
            linux_prot |= libc::PROT_NONE;
        }

        let ptr = unsafe { mmap(hint_addr, (page_count * 4095) as usize, linux_prot, linux_flags, -1, 0)}
            .map_err(linux_error_to_lilium)?;
        unsafe { core::ptr::write(base_addr, ptr); }
        Ok(())
    }
}

export_syscall! {
    unsafe extern fn RemoveMapping(base_addr: *mut c_void, page_count: isize) -> Result<()> {
        unsafe { munmap(base_addr, (page_count * 4096) as usize).map_err(linux_error_to_lilium)?;}
        Ok(())
    }
}

export_syscall! {
    unsafe extern fn ResizeMapping(base_addr: *mut c_void, old_page_count: isize, new_addr: *mut *mut c_void, new_page_count: isize) -> Result<()> {
        let mut flags = 0;
        if !new_addr.is_null() {
            flags |= libc::MREMAP_MAYMOVE;
        }

        let ptr = unsafe { mremap(base_addr, (old_page_count * 4096) as usize, (new_page_count * 4096) as usize, flags).map_err(linux_error_to_lilium)?};
        if !new_addr.is_null() {
            unsafe { write_checked(new_addr, ptr)?;}
        }

        Ok(())
    }
}

export_syscall! {
    unsafe extern fn ChangeMappingAttributes(base_addr: *mut c_void, page_count: isize, map_attrs: u32, _map_ext: *const KCSlice<sys::MapExtendedAttr>) -> Result<()> {
        let mut linux_prot = 0;
        if (map_attrs & !(sys::MAP_ATTR_READ | sys::MAP_ATTR_WRITE | sys::MAP_ATTR_EXEC | sys::MAP_ATTR_RESERVE)) != 0 {
            return Err(Error::InvalidOperation)
        }

        if (map_attrs & (sys::MAP_ATTR_WRITE | sys::MAP_ATTR_EXEC)) == (sys::MAP_ATTR_WRITE | sys::MAP_ATTR_EXEC) {
            // TODO: Check for MapExtendedAttrAllowWritableText
            return Err(Error::InvalidOperation)
        }

        if (map_attrs & sys::MAP_ATTR_READ) !=0 {
            linux_prot |= libc::PROT_READ;
        }

        if (map_attrs & sys::MAP_ATTR_WRITE) !=0  {
            linux_prot |= libc::PROT_WRITE;
        }

        if (map_attrs & sys::MAP_ATTR_EXEC) != 0 {
            linux_prot |= libc::PROT_EXEC
        }

        if (map_attrs & sys::MAP_ATTR_RESERVE) != 0 {
            linux_prot |= libc::PROT_NONE;
        }

        unsafe { mprotect(base_addr, (page_count * 4096) as usize, linux_prot).map_err(linux_error_to_lilium) }
    }
}
