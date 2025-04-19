use core::{
    cell::{Cell, UnsafeCell},
    ffi::c_long,
    num::{NonZero, NonZeroUsize},
};

use alloc::sync::Arc;

use indexmap::IndexSet;
use lilium_sys::{
    result::Result,
    sys::{
        handle::{self as sys, HANDLE_TYPE_IO, HandlePtr},
        kstr::KSlice,
        result::SysResult,
    },
};
use linux_raw_sys::general::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use rustix::fd::BorrowedFd;
use wl_interface_map::{GetInitHandlesTy, wl_get_init_handles_name};

use core::ffi::c_void;

#[repr(C, align(32))]
#[derive(bytemuck::Zeroable)]
pub struct Handle {
    ty: usize,
    blob1: *mut c_void,
    blob2: *mut c_void,
    fd: c_long,
}

#[thread_local]
static HANDLE_ARRAY: [UnsafeCell<Handle>; 512] =
    [const { UnsafeCell::new(bytemuck::zeroed()) }; 512];

#[thread_local]
static START_HINT: Cell<usize> = Cell::new(0);

pub fn insert_handle(handle: Handle) -> Result<HandlePtr<Handle>> {
    let start = START_HINT.get();
    for n in (0..512).map(|n| (n + start) & 511) {
        let h = &HANDLE_ARRAY[n];
        let v = h.get();

        if unsafe { v.cast::<usize>().read() } == 0 {
            START_HINT.set(n);
            unsafe {
                v.write(handle);
            }

            return Ok(unsafe { core::mem::transmute(v) });
        }
    }

    Err(lilium_sys::result::Error::ResourceLimitExhausted)
}

impl Drop for Handle {
    fn drop(&mut self) {}
}

impl Handle {
    pub unsafe fn try_deref<'a>(ptr: HandlePtr<Handle>) -> Result<&'a mut Handle> {
        let ptr: *mut Handle = unsafe { core::mem::transmute(ptr) };
        if HANDLE_ARRAY
            .as_ptr_range()
            .contains(&(ptr.cast_const().cast()))
            && ptr.is_aligned()
        {
            let v = ptr.cast::<usize>();
            if unsafe { core::ptr::read(v) } != 0 {
                return Ok(unsafe { &mut *ptr });
            }
        }
        Err(lilium_sys::result::Error::InvalidHandle)
    }

    pub fn ident(&self) -> usize {
        self.ty
    }

    pub fn borrow_fd(&self) -> Option<BorrowedFd> {
        if self.fd >= 0 {
            Some(unsafe { BorrowedFd::borrow_raw(self.fd as i32) })
        } else {
            None
        }
    }

    pub fn check_type(&self, ty: usize, mask: usize) -> Result<()> {
        if (self.ty & !mask) == ty {
            Ok(())
        } else {
            Err(lilium_sys::result::Error::InvalidHandle)
        }
    }
}

#[unsafe(export_name = wl_get_init_handles_name!())]
unsafe extern "C" fn get_init_handles(kslice: &mut KSlice<HandlePtr<sys::Handle>>) {
    // TODO: take full init handle array
    let stdin = Handle {
        ty: HANDLE_TYPE_IO as usize,
        blob1: core::ptr::null_mut(),
        blob2: core::ptr::null_mut(),
        fd: STDIN_FILENO as i64,
    };
    let stdout = Handle {
        ty: HANDLE_TYPE_IO as usize,
        blob1: core::ptr::null_mut(),
        blob2: core::ptr::null_mut(),
        fd: STDOUT_FILENO as i64,
    };
    let stderr = Handle {
        ty: HANDLE_TYPE_IO as usize,
        blob1: core::ptr::null_mut(),
        blob2: core::ptr::null_mut(),
        fd: STDERR_FILENO as i64,
    };
    let sl = unsafe { kslice.as_slice_mut() };
    sl[0] = insert_handle(stdin).unwrap().cast();
    sl[1] = insert_handle(stdout).unwrap().cast();
    sl[2] = insert_handle(stderr).unwrap().cast();
    kslice.len = 3;
}

const _: GetInitHandlesTy = get_init_handles;
