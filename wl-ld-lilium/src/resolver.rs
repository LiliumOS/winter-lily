use core::ffi::c_void;
use ld_so_impl::resolver::Resolver;

use crate::ldso::{self, SearchType};

pub fn lookup_soname(libname: &core::ffi::CStr, resolver: &'static Resolver, udata: *mut c_void) {
    // Safety: `lookup_soname` is only ever called using a provided udata, which is `core::ptr::without_provenance(SearchType as usize)`
    let search: SearchType = unsafe { core::mem::transmute(udata) };

    let (search, nname) = if search == SearchType::Winter {
        match libname.to_bytes() {
            b"libusi-base.so" | b"libusi-thread.so" | b"libusi-io.so" | b"libusi-process.so"
            | b"libusi-debug.so" | b"libusi-kmgmt.so" => {
                // default subsystem, preloaded before any lilium code is loaded
                return;
            }
            b"libusi-rtld.so" | b"libusi-unwind.so" | b"libusi-vti.so" | b"libc.so"
            | b"libdl.so" => {
                let redirect_name = match libname.to_bytes() {
                    b"libusi-rtld.so" => c"libwl-usi-rtld.so",
                    b"libusi-unwind.so" => c"libwl-usi-unwind.so",
                    b"libusi-vti.so" => c"libwl-usi-vti.so",
                    b"libc.so" => c"libwl-usi-posix.so",
                    b"libdl.so" => c"libwl-usi-dl.so",
                    _ => todo!(),
                };

                (SearchType::Host, redirect_name)
            }
            _ => (search, libname),
        }
    } else {
        (search, libname)
    };

    match ldso::open_module(search, nname) {
        Ok(fd) => {}
        Err(_) => resolver.resolve_error(libname),
    }
}
