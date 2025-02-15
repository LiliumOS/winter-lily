use std::{cell::UnsafeCell, num::NonZero, sync::Arc};

use indexmap::IndexSet;
use lilium_sys::{
    result::Result,
    sys::{
        handle::{Handle, HandlePtr},
        result::SysResult,
    },
};

use core::ffi::c_void;

use crate::helpers::as_ptr_range;

pub trait HandleRights {
    fn grant_right(&mut self, right: &str) -> Result<()>;
    fn check_right(&self, right: &str) -> Result<()>;
}

pub struct HandleBlob {
    h_type: NonZero<usize>,
    rights: Arc<dyn HandleRights + 'static + Sync>,
    h_data: *mut c_void,
    shared_entry: *mut c_void,
}

impl HandleBlob {
    pub fn handle_type(&self) -> usize {
        self.h_type.get()
    }

    pub fn data(&self) -> *mut c_void {
        self.h_data
    }

    /// Shortcut for [`HandleBlob::data`]. Used when the handle stores an fd or pid.
    pub fn data_int(&self) -> usize {
        self.h_data.addr()
    }
}

#[thread_local]
static HANDLE_BLOCK: UnsafeCell<[Option<HandleBlob>; 512]> = UnsafeCell::new([const { None }; 512]);

pub unsafe fn deref<'a, H: 'static>(p: HandlePtr<H>) -> Option<Result<&'a HandleBlob>> {
    let ptr: *mut H = unsafe { core::mem::transmute(p) };
    if ptr.is_null() {
        None
    } else if !unsafe { as_ptr_range(HANDLE_BLOCK.get()) }.contains(&ptr.cast_const().cast()) {
        Some(Err(lilium_sys::result::Error::InvalidHandle))
    } else if unsafe { ptr.cast::<usize>().read() } == 0 {
        Some(Err(lilium_sys::result::Error::InvalidHandle))
    } else {
        Some(Ok(unsafe { &*ptr.cast() }))
    }
}

crate::export_syscall! {
    unsafe extern fn IdentHandle(hdl: HandlePtr<Handle>) -> Result<usize> {
        let hdl = unsafe{ deref(hdl) }
            .ok_or_else(|| lilium_sys::result::Error::InvalidHandle)
            .and_then(|r| r);
        hdl.map(|h| h.handle_type())
    }
}
