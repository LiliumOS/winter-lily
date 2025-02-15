use std::ffi::c_void;

#[cfg(target_pointer_width = "64")]
#[repr(C)]
pub struct AuxEnt {
    pub at_tag: usize,
    pub at_val: *mut c_void,
}
