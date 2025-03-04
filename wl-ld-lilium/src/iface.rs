use core::ffi::c_void;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _ITM_deregisterTMCloneTable(_: *mut c_void) {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _ITM_registerTMCloneTable(_: *mut c_void, _: usize) {}
