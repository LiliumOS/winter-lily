use core::ffi::c_void;
use ld_so_impl::resolver::Resolver;

use crate::ldso::{self, SearchType};

pub fn lookup_soname(libname: &core::ffi::CStr, resolver: &'static Resolver, udata: *mut c_void) {
    todo!()
}
