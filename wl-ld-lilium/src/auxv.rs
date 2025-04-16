use core::ffi::c_void;

#[cfg(target_pointer_width = "64")]
#[repr(C)]
#[derive(bytemuck::Zeroable)]
pub struct AuxEnt {
    pub at_tag: usize,
    pub at_val: *mut c_void,
}
unsafe impl Sync for AuxEnt {}
